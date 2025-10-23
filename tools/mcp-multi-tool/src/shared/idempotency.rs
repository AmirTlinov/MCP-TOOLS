use crate::shared::types::InspectionRunEvent;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
enum Record {
    InFlight {
        claimed_at: Instant,
    },
    Completed {
        claimed_at: Instant,
        event: InspectionRunEvent,
    },
}

#[derive(Debug, Default)]
pub struct IdempotencyStore {
    records: Mutex<HashMap<String, Record>>,
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
        }
    }

    pub fn claim(&self, key: &str) -> ClaimOutcome {
        let mut store = self.records.lock();
        match store.get(key) {
            Some(Record::InFlight { .. }) => ClaimOutcome::InFlight,
            Some(Record::Completed { event, .. }) => ClaimOutcome::Completed(event.clone()),
            None => {
                store.insert(
                    key.to_string(),
                    Record::InFlight {
                        claimed_at: Instant::now(),
                    },
                );
                ClaimOutcome::Accepted
            }
        }
    }

    pub fn complete(&self, key: &str, event: InspectionRunEvent) {
        let mut store = self.records.lock();
        store.insert(
            key.to_string(),
            Record::Completed {
                claimed_at: Instant::now(),
                event,
            },
        );
    }

    pub fn reap_expired(&self, ttl: Duration) {
        let mut store = self.records.lock();
        store.retain(|_, record| match record {
            Record::InFlight { claimed_at } => claimed_at.elapsed() <= ttl,
            Record::Completed { claimed_at, .. } => claimed_at.elapsed() <= ttl,
        });
    }
}
