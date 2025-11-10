use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        (SANDBOX_STATE_INITIALIZING, SANDBOX_STATE_IDLE) => true,
        (SANDBOX_STATE_INITIALIZING, SANDBOX_STATE_BUSY) => true,
        (SANDBOX_STATE_INITIALIZING, SANDBOX_STATE_TERMINATING) => true,
        (SANDBOX_STATE_IDLE, SANDBOX_STATE_BUSY) => true,
        (SANDBOX_STATE_IDLE, SANDBOX_STATE_TERMINATING) => true,
        (SANDBOX_STATE_IDLE, SANDBOX_STATE_TERMINATED) => true,
        (SANDBOX_STATE_BUSY, SANDBOX_STATE_IDLE) => true,
        (SANDBOX_STATE_BUSY, SANDBOX_STATE_TERMINATING) => true,
        (SANDBOX_STATE_BUSY, SANDBOX_STATE_TERMINATED) => true,
        (SANDBOX_STATE_TERMINATING, SANDBOX_STATE_TERMINATED) => true,
        _ => false,
    }
}
