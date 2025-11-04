use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        (SESSION_STATE_INIT, SESSION_STATE_IDLE) => true,

        // From IDLE (ready and waiting)
        (SESSION_STATE_IDLE, SESSION_STATE_STOPPED) => true, // User stops or controller detects failure
        (SESSION_STATE_IDLE, SESSION_STATE_BUSY) => true,    // Processing request

        // From STOPPED (container destroyed, volume preserved)
        (SESSION_STATE_STOPPED, SESSION_STATE_IDLE) => true, // User restarts, recreate container

        // From BUSY (actively processing)
        (SESSION_STATE_BUSY, SESSION_STATE_IDLE) => true, // Processing complete
        (SESSION_STATE_BUSY, SESSION_STATE_STOPPED) => true, // Controller detects container failure

        // Terminal states are not enforced here; hard delete removes the record instead

        // Cannot transition to same state
        _ => false,
    }
}
