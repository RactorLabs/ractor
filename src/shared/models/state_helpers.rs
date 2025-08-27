use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        (SESSION_STATE_INIT, SESSION_STATE_IDLE) => true,
        (SESSION_STATE_INIT, SESSION_STATE_ERRORED) => true,
        
        // From IDLE (ready and waiting)
        (SESSION_STATE_IDLE, SESSION_STATE_CLOSED) => true,  // User suspends
        (SESSION_STATE_IDLE, SESSION_STATE_BUSY) => true,  // Processing request
        (SESSION_STATE_IDLE, SESSION_STATE_ERRORED) => true,
        
        // From SUSPENDED (container destroyed, volume preserved)
        (SESSION_STATE_CLOSED, SESSION_STATE_IDLE) => true,  // User resumes, recreate container
        (SESSION_STATE_CLOSED, SESSION_STATE_ERRORED) => true,
        
        // From BUSY (actively processing)
        (SESSION_STATE_BUSY, SESSION_STATE_IDLE) => true,  // Processing complete
        (SESSION_STATE_BUSY, SESSION_STATE_ERRORED) => true,
        
        // Terminal states: error and deleted
        // These states have no outgoing transitions (can only be remixed)
        
        // Cannot transition to same state
        _ => false,
    }
}