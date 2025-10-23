use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Pending,
    Processing,
    Captured,
    Failed,
}

#[derive(Debug, Clone)]
pub struct InspectionRun {
    pub id: uuid::Uuid,
    pub state: RunState,
}

impl InspectionRun {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            state: RunState::Pending,
        }
    }
    pub fn start(&mut self) {
        assert!(matches!(self.state, RunState::Pending));
        self.state = RunState::Processing;
    }
    pub fn capture(&mut self) {
        assert!(matches!(self.state, RunState::Processing));
        self.state = RunState::Captured;
    }
    pub fn fail(&mut self) {
        assert!(!matches!(self.state, RunState::Captured));
        self.state = RunState::Failed;
    }
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::Pending => "pending",
            RunState::Processing => "processing",
            RunState::Captured => "captured",
            RunState::Failed => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_transitions() {
        let mut r = InspectionRun::new();
        assert!(matches!(r.state, RunState::Pending));
        r.start();
        assert!(matches!(r.state, RunState::Processing));
        r.capture();
        assert!(matches!(r.state, RunState::Captured));
    }

    #[test]
    #[should_panic]
    fn no_skip_states() {
        let mut r = InspectionRun::new();
        r.capture();
    }
}
