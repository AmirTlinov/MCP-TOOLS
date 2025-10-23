use crate::shared::types::{CallRequest, InspectionRunEvent, TargetDescriptor};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
struct InFlightRecord {
    claimed_at: Instant,
    run_id: Option<uuid::Uuid>,
    request: Option<CallRequest>,
    target: Option<TargetDescriptor>,
    started_at: Option<OffsetDateTime>,
}

impl InFlightRecord {
    fn new() -> Self {
        Self {
            claimed_at: Instant::now(),
            run_id: None,
            request: None,
            target: None,
            started_at: None,
        }
    }
}

#[derive(Debug)]
enum Record {
    InFlight(InFlightRecord),
    Completed {
        claimed_at: Instant,
        event: InspectionRunEvent,
    },
}

#[derive(Debug, Clone)]
struct ExternalRecord {
    recorded_at: Instant,
    event: InspectionRunEvent,
}

#[derive(Debug, Clone)]
pub struct ReapedEvent {
    pub idempotency_key: String,
    pub event: InspectionRunEvent,
}

#[derive(Debug, Default)]
pub struct IdempotencyStore {
    records: Mutex<HashMap<String, Record>>,
    external_refs: Mutex<HashMap<String, ExternalRecord>>,
}

pub enum ClaimOutcome {
    Accepted,
    InFlight,
    Completed(InspectionRunEvent),
}

impl IdempotencyStore {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            external_refs: Mutex::new(HashMap::new()),
        }
    }

    pub fn claim(&self, key: &str) -> ClaimOutcome {
        let mut store = self.records.lock();
        match store.get(key) {
            Some(Record::InFlight(_)) => ClaimOutcome::InFlight,
            Some(Record::Completed { event, .. }) => ClaimOutcome::Completed(event.clone()),
            None => {
                store.insert(key.to_string(), Record::InFlight(InFlightRecord::new()));
                ClaimOutcome::Accepted
            }
        }
    }

    pub fn begin(&self, key: &str, run_id: uuid::Uuid, request: &CallRequest) {
        let mut store = self.records.lock();
        let entry = store
            .entry(key.to_string())
            .or_insert_with(|| Record::InFlight(InFlightRecord::new()));
        if let Record::InFlight(record) = entry {
            record.run_id = Some(run_id);
            record.request = Some(request.clone());
        }
    }

    pub fn mark_started(&self, key: &str, started_at: OffsetDateTime) {
        let mut store = self.records.lock();
        if let Some(Record::InFlight(record)) = store.get_mut(key) {
            record.started_at = Some(started_at);
        }
    }

    pub fn set_target(&self, key: &str, target: TargetDescriptor) {
        let mut store = self.records.lock();
        if let Some(Record::InFlight(record)) = store.get_mut(key) {
            record.target = Some(target);
        }
    }

    pub fn complete(&self, key: &str, event: InspectionRunEvent) {
        let mut store = self.records.lock();
        store.insert(
            key.to_string(),
            Record::Completed {
                claimed_at: Instant::now(),
                event: event.clone(),
            },
        );
        drop(store);
        if let Some(reference) = event.external_reference.clone() {
            self.record_external_ref(&reference, event);
        }
    }

    pub fn reap_expired(&self, ttl: Duration, now: OffsetDateTime) -> Vec<ReapedEvent> {
        let mut store = self.records.lock();
        let mut expired: Vec<(String, InspectionRunEvent)> = Vec::new();

        store.retain(|key, record| match record {
            Record::InFlight(record) => {
                if record.claimed_at.elapsed() > ttl {
                    if let Some(event) = build_timeout_event(key, record, now) {
                        expired.push((key.clone(), event));
                    }
                    false
                } else {
                    true
                }
            }
            Record::Completed { claimed_at, .. } => claimed_at.elapsed() <= ttl,
        });

        let mut results = Vec::new();
        for (key, event) in expired {
            store.insert(
                key.clone(),
                Record::Completed {
                    claimed_at: Instant::now(),
                    event: event.clone(),
                },
            );
            results.push(ReapedEvent {
                idempotency_key: key,
                event,
            });
        }
        drop(store);

        let mut external = self.external_refs.lock();
        external.retain(|_, record| record.recorded_at.elapsed() <= ttl);
        drop(external);

        for reaped in &results {
            if let Some(reference) = reaped.event.external_reference.clone() {
                self.record_external_ref(&reference, reaped.event.clone());
            }
        }

        results
    }

    pub fn find_external_ref(&self, reference: &str) -> Option<InspectionRunEvent> {
        let store = self.external_refs.lock();
        store.get(reference).map(|record| record.event.clone())
    }

    pub fn record_external_ref(&self, reference: &str, event: InspectionRunEvent) {
        let mut store = self.external_refs.lock();
        store.insert(
            reference.to_string(),
            ExternalRecord {
                recorded_at: Instant::now(),
                event,
            },
        );
    }
}

fn build_timeout_event(
    key: &str,
    record: &InFlightRecord,
    now: OffsetDateTime,
) -> Option<InspectionRunEvent> {
    let run_id = record.run_id?;
    let request = record.request.as_ref()?;
    let started_at = record.started_at.unwrap_or(now);
    let mut duration = now - started_at;
    if duration.is_negative() {
        duration = time::Duration::ZERO;
    }
    let duration_ms = duration.whole_milliseconds() as u64;

    Some(InspectionRunEvent {
        event_id: uuid::Uuid::new_v4(),
        run_id,
        tool_name: request.tool_name.clone(),
        state: "failed".into(),
        started_at: started_at.to_string(),
        duration_ms,
        target: record.target.clone(),
        request: serde_json::to_value(request).ok(),
        response: None,
        error: Some(format!(
            "run timed out after {} ms (idempotency key {})",
            record.claimed_at.elapsed().as_millis(),
            key
        )),
        idempotency_key: request.idempotency_key.clone(),
        external_reference: request.external_reference.clone(),
    })
}
