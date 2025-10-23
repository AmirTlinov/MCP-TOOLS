use std::sync::Arc;

use mcp_multi_tool::shared::idempotency::{ClaimOutcome, IdempotencyStore};
use mcp_multi_tool::shared::types::{InspectionRunEvent, TargetDescriptor};
use tokio::task::JoinSet;
use uuid::Uuid;

fn dummy_event() -> InspectionRunEvent {
    InspectionRunEvent {
        event_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        tool_name: "dummy".into(),
        state: "captured".into(),
        started_at: "2025-01-01T00:00:00Z".into(),
        duration_ms: 42,
        target: Some(TargetDescriptor {
            transport: "stdio".into(),
            command: Some("dummy".into()),
            url: None,
            headers: None,
        }),
        request: None,
        response: None,
        error: None,
    }
}

#[test]
fn idempotency_state_machine() {
    let store = IdempotencyStore::new();
    let key = "k1";
    match store.claim(key) {
        ClaimOutcome::Accepted => {}
        _ => panic!("expected Accepted"),
    }
    match store.claim(key) {
        ClaimOutcome::InFlight => {}
        _ => panic!("expected InFlight"),
    }
    store.complete(key, dummy_event());
    match store.claim(key) {
        ClaimOutcome::Completed(event) => {
            assert_eq!(event.tool_name, "dummy");
        }
        ClaimOutcome::InFlight => panic!("expected completion"),
        ClaimOutcome::Accepted => panic!("claim should not be accepted twice"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn idempotency_concurrency_one_winner() {
    let store = Arc::new(IdempotencyStore::new());
    let key = "race-key";
    let mut tasks = JoinSet::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.spawn(async move { store.claim(key) });
    }
    let mut accepted = 0;
    while let Some(outcome) = tasks.join_next().await {
        match outcome.expect("task") {
            ClaimOutcome::Accepted => accepted += 1,
            ClaimOutcome::InFlight => {}
            ClaimOutcome::Completed(_) => {}
        }
    }
    assert_eq!(accepted, 1, "only one caller should win the claim");
    store.complete(key, dummy_event());
    match store.claim(key) {
        ClaimOutcome::Completed(_) => {}
        ClaimOutcome::InFlight => panic!("run still in-flight after completion"),
        ClaimOutcome::Accepted => panic!("claim reopened unexpectedly"),
    }
}
