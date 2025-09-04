use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        (AGENT_STATE_INIT, AGENT_STATE_IDLE) => true,
        (AGENT_STATE_INIT, AGENT_STATE_ERRORED) => true,

        // From IDLE (ready and waiting)
        (AGENT_STATE_IDLE, AGENT_STATE_CLOSED) => true, // User suspends
        (AGENT_STATE_IDLE, AGENT_STATE_BUSY) => true,   // Processing request
        (AGENT_STATE_IDLE, AGENT_STATE_ERRORED) => true,

        // From SUSPENDED (container destroyed, volume preserved)
        (AGENT_STATE_CLOSED, AGENT_STATE_IDLE) => true, // User resumes, recreate container
        (AGENT_STATE_CLOSED, AGENT_STATE_ERRORED) => true,

        // From BUSY (actively processing)
        (AGENT_STATE_BUSY, AGENT_STATE_IDLE) => true, // Processing complete
        (AGENT_STATE_BUSY, AGENT_STATE_ERRORED) => true,

        // Terminal states: error and deleted
        // These states have no outgoing transitions (can only be remixed)

        // Cannot transition to same state
        _ => false,
    }
}
