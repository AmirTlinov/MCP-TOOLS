use std::{
    collections::HashMap,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use anyhow::Result;
use mcp_multi_tool::{
    app::error_budget::{
        ErrorBudget, ErrorBudgetParams, RecordOutcome,
        configure_lock_observer as configure_error_budget_observer,
    },
    infra::{metrics, outbox::Outbox},
    shared::{
        idempotency::{
            ClaimOutcome, IdempotencyStore,
            configure_lock_observer as configure_idempotency_observer,
        },
        types::{CallRequest, InspectionRunEvent, TargetDescriptor},
    },
};
use tempfile::tempdir;
use time::OffsetDateTime;
use uuid::Uuid;

const LOCK_P99_THRESHOLD_MS: f64 = 50.0;

#[test]
fn idempotency_lock_wait_p99_within_budget() {
    configure_idempotency_observer(metrics::observe_lock_wait);
    configure_error_budget_observer(metrics::observe_lock_wait);
    metrics::take_lock_wait_records();
    let store = Arc::new(IdempotencyStore::new());
    thread::scope(|scope| {
        for t in 0..32 {
            let store = Arc::clone(&store);
            scope.spawn(move || {
                for i in 0..256 {
                    let key = format!("lock-idempotency-{t}-{i}");
                    if let ClaimOutcome::Accepted = store.claim(&key) {
                        let req = CallRequest {
                            tool_name: "help".into(),
                            arguments_json: serde_json::json!({}),
                            idempotency_key: Some(key.clone()),
                            stream: false,
                            external_reference: None,
                            stdio: None,
                            sse: None,
                            http: None,
                        };
                        let run_id = Uuid::new_v4();
                        store.begin(&key, run_id, &req);
                        let started_at = OffsetDateTime::now_utc();
                        store.mark_started(&key, started_at);
                        store.set_target(
                            &key,
                            TargetDescriptor {
                                transport: "stdio".into(),
                                command: Some("mock".into()),
                                url: None,
                                headers: None,
                            },
                        );
                        let event = InspectionRunEvent {
                            event_id: Uuid::new_v4(),
                            run_id,
                            tool_name: "help".into(),
                            state: "captured".into(),
                            started_at: started_at.to_string(),
                            duration_ms: 5,
                            target: None,
                            request: None,
                            response: None,
                            error: None,
                            idempotency_key: Some(key.clone()),
                            external_reference: None,
                        };
                        store.complete(&key, event);
                    }
                }
            });
        }
    });

    let records = metrics::take_lock_wait_records();
    assert_component_p99(&records, "idempotency_records");
    assert_component_p99(&records, "idempotency_external");
}

#[test]
fn outbox_lock_wait_p99_within_budget() -> Result<()> {
    configure_idempotency_observer(metrics::observe_lock_wait);
    metrics::take_lock_wait_records();
    let dir = tempdir()?;
    let db_path = dir.path().join("outbox.db");
    let dlq = dir.path().join("dlq.jsonl");
    let outbox = Arc::new(Outbox::sqlite(&db_path, &dlq)?);
    thread::scope(|scope| {
        for t in 0..32 {
            let outbox = Arc::clone(&outbox);
            scope.spawn(move || {
                for i in 0..128 {
                    let payload = serde_json::json!({
                        "event_id": Uuid::new_v4(),
                        "payload": format!("outbox-{t}-{i}"),
                    });
                    outbox.append(&payload).expect("append");
                }
            });
        }
    });

    let records = metrics::take_lock_wait_records();
    assert_component_p99(&records, "outbox_write_lock");
    assert_component_p99(&records, "outbox_sqlite_conn");
    Ok(())
}

#[test]
fn error_budget_lock_wait_p99_within_budget() {
    configure_error_budget_observer(metrics::observe_lock_wait);
    metrics::take_lock_wait_records();
    let params = ErrorBudgetParams {
        enabled: true,
        success_threshold: 0.5,
        minimum_requests: 10,
        sample_window: Duration::from_secs(60),
        freeze_duration: Duration::from_secs(30),
    };
    let budget = Arc::new(ErrorBudget::new(params));
    thread::scope(|scope| {
        for t in 0..32 {
            let budget = Arc::clone(&budget);
            scope.spawn(move || {
                for i in 0..128 {
                    let now = SystemTime::now() + Duration::from_millis(((t * 128 + i) as u64) % 5);
                    match budget.record(i % 3 != 0, now) {
                        RecordOutcome::FreezeTriggered(_) | RecordOutcome::FreezeCleared => {
                            metrics::set_error_budget_frozen(false)
                        }
                        RecordOutcome::None => {}
                    }
                }
            });
        }
    });

    let records = metrics::take_lock_wait_records();
    assert_component_p99(&records, "error_budget_state");
}

fn assert_component_p99(records: &HashMap<String, Vec<f64>>, component: &str) {
    if let Some(samples) = records.get(component) {
        if samples.is_empty() {
            return;
        }
        let p99 = percentile(samples.clone(), 99.0);
        assert!(
            p99 <= LOCK_P99_THRESHOLD_MS,
            "component {component} p99 {:.2}ms exceeds {:.2}ms",
            p99,
            LOCK_P99_THRESHOLD_MS
        );
    }
}

fn percentile(mut data: Vec<f64>, target: f64) -> f64 {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = data.len();
    if n == 0 {
        return 0.0;
    }
    let rank = ((target / 100.0) * (n - 1) as f64).ceil() as usize;
    let index = rank.min(n - 1);
    data[index]
}
