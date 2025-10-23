use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Clone)]
pub struct ErrorBudgetParams {
    pub enabled: bool,
    pub success_threshold: f64,
    pub minimum_requests: usize,
    pub sample_window: Duration,
    pub freeze_duration: Duration,
}

impl ErrorBudgetParams {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            success_threshold: 1.0,
            minimum_requests: 0,
            sample_window: Duration::from_secs(0),
            freeze_duration: Duration::from_secs(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FreezeReport {
    pub until: SystemTime,
    pub success_rate: f64,
    pub sample_size: usize,
}

#[derive(Debug)]
struct Observation {
    at: SystemTime,
    success: bool,
}

#[derive(Debug, Default)]
struct ErrorBudgetState {
    observations: VecDeque<Observation>,
    frozen_until: Option<SystemTime>,
}

#[derive(Debug)]
pub struct ErrorBudget {
    params: ErrorBudgetParams,
    state: Mutex<ErrorBudgetState>,
}

type LockObserver = fn(&'static str, Duration);

static LOCK_OBSERVER: Lazy<RwLock<Option<LockObserver>>> = Lazy::new(|| RwLock::new(None));

fn record_lock_wait(component: &'static str, waited: Duration) {
    if let Some(observer) = *LOCK_OBSERVER.read() {
        observer(component, waited);
    }
}

pub fn configure_lock_observer(observer: LockObserver) {
    *LOCK_OBSERVER.write() = Some(observer);
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordOutcome {
    None,
    FreezeTriggered(FreezeReport),
    FreezeCleared,
}

impl ErrorBudget {
    pub fn new(params: ErrorBudgetParams) -> Self {
        if params.enabled {
            assert!(params.success_threshold > 0.0 && params.success_threshold <= 1.0);
            assert!(params.minimum_requests > 0);
            assert!(params.sample_window > Duration::from_secs(0));
            assert!(params.freeze_duration > Duration::from_secs(0));
        }
        Self {
            params,
            state: Mutex::new(ErrorBudgetState::default()),
        }
    }

    pub fn disabled() -> Self {
        Self::new(ErrorBudgetParams::disabled())
    }

    pub fn admit_now(&self) -> Result<bool, FreezeReport> {
        self.admit(SystemTime::now())
    }

    pub fn admit(&self, now: SystemTime) -> Result<bool, FreezeReport> {
        if !self.params.enabled {
            return Ok(false);
        }
        let wait = Instant::now();
        let mut state = self.state.lock();
        record_lock_wait("error_budget_state", wait.elapsed());
        self.purge_old(now, &mut state);
        if let Some(until) = state.frozen_until {
            if now < until {
                let (success_rate, sample_size) = self.current_success_rate(&state);
                return Err(FreezeReport {
                    until,
                    success_rate,
                    sample_size,
                });
            }
            state.frozen_until = None;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn record_success_now(&self) -> RecordOutcome {
        self.record(true, SystemTime::now())
    }

    pub fn record_failure_now(&self) -> RecordOutcome {
        self.record(false, SystemTime::now())
    }

    pub fn record(&self, success: bool, now: SystemTime) -> RecordOutcome {
        if !self.params.enabled {
            return RecordOutcome::None;
        }
        let wait = Instant::now();
        let mut state = self.state.lock();
        record_lock_wait("error_budget_state", wait.elapsed());
        self.purge_old(now, &mut state);

        let mut thawed = false;
        if let Some(until) = state.frozen_until {
            if now >= until {
                state.frozen_until = None;
                thawed = true;
            }
        }

        state
            .observations
            .push_back(Observation { at: now, success });

        if thawed {
            return RecordOutcome::FreezeCleared;
        }

        if state.frozen_until.is_some() {
            return RecordOutcome::None;
        }

        let (success_rate, sample_size) = self.current_success_rate(&state);
        if sample_size >= self.params.minimum_requests
            && success_rate < self.params.success_threshold
        {
            let until = now + self.params.freeze_duration;
            state.frozen_until = Some(until);
            RecordOutcome::FreezeTriggered(FreezeReport {
                until,
                success_rate,
                sample_size,
            })
        } else {
            RecordOutcome::None
        }
    }

    fn purge_old(&self, now: SystemTime, state: &mut ErrorBudgetState) {
        let window = self.params.sample_window;
        while let Some(front) = state.observations.front() {
            if now
                .duration_since(front.at)
                .map_or(false, |age| age > window)
            {
                state.observations.pop_front();
            } else {
                break;
            }
        }
    }

    fn current_success_rate(&self, state: &ErrorBudgetState) -> (f64, usize) {
        if state.observations.is_empty() {
            return (1.0, 0);
        }
        let total = state.observations.len();
        let success = state.observations.iter().filter(|obs| obs.success).count();
        (success as f64 / total as f64, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn params() -> ErrorBudgetParams {
        ErrorBudgetParams {
            enabled: true,
            success_threshold: 0.8,
            minimum_requests: 3,
            sample_window: Duration::from_secs(120),
            freeze_duration: Duration::from_secs(30),
        }
    }

    fn ts(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn success_only_never_freezes() {
        let budget = ErrorBudget::new(params());
        assert_eq!(budget.admit(ts(0)), Ok(false));
        assert_eq!(budget.record(true, ts(1)), RecordOutcome::None);
        assert_eq!(budget.record(true, ts(2)), RecordOutcome::None);
        assert_eq!(budget.record(true, ts(3)), RecordOutcome::None);
        assert_eq!(budget.admit(ts(4)), Ok(false));
    }

    #[test]
    fn failures_trigger_freeze() {
        let budget = ErrorBudget::new(params());
        assert_eq!(budget.record(false, ts(1)), RecordOutcome::None);
        assert_eq!(budget.record(false, ts(2)), RecordOutcome::None);
        match budget.record(false, ts(3)) {
            RecordOutcome::FreezeTriggered(freeze) => {
                assert!(freeze.success_rate < 0.8);
                assert_eq!(freeze.sample_size, 3);
            }
            other => panic!("expected freeze trigger, got {:?}", other),
        }
        let freeze = budget.admit(ts(4)).unwrap_err();
        assert!(freeze.until > ts(4));
    }

    #[test]
    fn freeze_expires_after_window() {
        let mut params = params();
        params.freeze_duration = Duration::from_secs(10);
        let budget = ErrorBudget::new(params);
        assert_eq!(budget.record(false, ts(1)), RecordOutcome::None);
        assert_eq!(budget.record(false, ts(2)), RecordOutcome::None);
        assert!(matches!(
            budget.record(false, ts(3)),
            RecordOutcome::FreezeTriggered(_)
        ));
        assert!(budget.admit(ts(4)).is_err());
        assert_eq!(budget.admit(ts(20)), Ok(true));
        match budget.record(true, ts(21)) {
            RecordOutcome::FreezeTriggered(_)
            | RecordOutcome::FreezeCleared
            | RecordOutcome::None => {}
        }
    }
}
