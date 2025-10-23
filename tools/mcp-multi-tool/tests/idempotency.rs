use std::sync::Arc;

use mcp_multi_tool::shared::idempotency::{ClaimOutcome, IdempotencyStore};
use mcp_multi_tool::shared::types::{CallRequest, InspectionRunEvent, TargetDescriptor};
use proptest::prelude::*;
use serde_json::json;
use std::{thread, time::Duration};
use time::OffsetDateTime;
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
        idempotency_key: None,
        external_reference: None,
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

#[test]
fn external_reference_deduplicates() {
    let store = IdempotencyStore::new();
    let mut event = dummy_event();
    event.external_reference = Some("ext-1".into());
    store.record_external_ref("ext-1", event.clone());
    let duplicate = store.find_external_ref("ext-1");
    assert!(duplicate.is_some());
    let fetched = duplicate.unwrap();
    assert_eq!(fetched.external_reference.as_deref(), Some("ext-1"));
}

#[test]
fn reap_expired_prunes_external_refs() {
    let store = IdempotencyStore::new();
    let mut event = dummy_event();
    event.external_reference = Some("ext-prune".into());
    store.record_external_ref("ext-prune", event);
    thread::sleep(Duration::from_millis(5));
    store.reap_expired(Duration::from_millis(1), OffsetDateTime::now_utc());
    assert!(store.find_external_ref("ext-prune").is_none());
}

#[test]
fn reaper_emits_failure_event() {
    let store = IdempotencyStore::new();
    let key = "reaper-key";
    let request = CallRequest {
        tool_name: "demo".into(),
        arguments_json: json!({}),
        idempotency_key: Some(key.into()),
        external_reference: Some("ext-demo".into()),
        stdio: None,
        sse: None,
        http: None,
    };

    assert!(matches!(store.claim(key), ClaimOutcome::Accepted));
    let run_id = Uuid::new_v4();
    store.begin(key, run_id, &request);
    let started_at = OffsetDateTime::now_utc() - time::Duration::milliseconds(10);
    store.mark_started(key, started_at);
    store.set_target(
        key,
        TargetDescriptor {
            transport: "stdio".into(),
            command: Some("demo".into()),
            url: None,
            headers: None,
        },
    );

    let reaped = store.reap_expired(Duration::from_millis(0), OffsetDateTime::now_utc());
    assert_eq!(reaped.len(), 1);
    let event = &reaped[0].event;
    assert_eq!(event.tool_name, "demo");
    assert_eq!(event.state, "failed");
    assert!(event.error.as_ref().unwrap().contains("timed out"));
    assert_eq!(event.external_reference.as_deref(), Some("ext-demo"));

    match store.claim(key) {
        ClaimOutcome::Completed(saved) => {
            assert_eq!(saved.tool_name, "demo");
        }
        _ => panic!("expected completed outcome"),
    }

    // ensure external reference recorded
    let existing = store
        .find_external_ref("ext-demo")
        .expect("external reference recorded");
    assert_eq!(existing.tool_name, "demo");
}

#[derive(Clone, Debug)]
enum Operation {
    Claim,
    Complete,
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, .. ProptestConfig::default() })]
    #[test]
    fn single_winner_per_claim_sequence(ops in proptest::collection::vec(prop_oneof![
        Just(Operation::Claim),
        Just(Operation::Complete),
    ], 1..128)) {
        let store = IdempotencyStore::new();
        let key = "prop-key";
        let mut accepted = 0u32;
        for op in ops {
            match op {
                Operation::Claim => match store.claim(key) {
                    ClaimOutcome::Accepted => accepted += 1,
                    ClaimOutcome::InFlight | ClaimOutcome::Completed(_) => {},
                },
                Operation::Complete => {
                    store.complete(key, dummy_event());
                }
            }
        }
        prop_assert!(accepted <= 1);
    }
}
