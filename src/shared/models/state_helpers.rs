use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        (SANDBOX_STATE_INIT, SANDBOX_STATE_IDLE) => true,
        (SANDBOX_STATE_IDLE, SANDBOX_STATE_DELETED) => true,
        (SANDBOX_STATE_IDLE, SANDBOX_STATE_BUSY) => true,
        (SANDBOX_STATE_BUSY, SANDBOX_STATE_IDLE) => true,
        (SANDBOX_STATE_BUSY, SANDBOX_STATE_DELETED) => true,
        _ => false,
    }
}
