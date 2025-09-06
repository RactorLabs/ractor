use super::constants::*;

pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        (AGENT_STATE_INIT, AGENT_STATE_IDLE) => true,

        // From IDLE (ready and waiting)
        (AGENT_STATE_IDLE, AGENT_STATE_SLEPT) => true, // User sleeps or controller detects failure
        (AGENT_STATE_IDLE, AGENT_STATE_BUSY) => true,   // Processing request

        // From SLEPT (container destroyed, volume preserved)
        (AGENT_STATE_SLEPT, AGENT_STATE_IDLE) => true, // User wakes, recreate container

        // From BUSY (actively processing)
        (AGENT_STATE_BUSY, AGENT_STATE_IDLE) => true, // Processing complete
        (AGENT_STATE_BUSY, AGENT_STATE_SLEPT) => true, // Controller detects container failure

        // Terminal state: deleted
        // This state has no outgoing transitions (can only be remixed)

        // Cannot transition to same state
        _ => false,
    }
}
