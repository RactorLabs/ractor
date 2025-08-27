pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        ("init", "idle") => true,
        ("init", "error") => true,
        
        // From IDLE (ready and waiting)
        ("idle", "closed") => true,  // User suspends
        ("idle", "busy") => true,  // Processing request
        ("idle", "error") => true,
        
        // From SUSPENDED (container destroyed, volume preserved)
        ("closed", "idle") => true,  // User resumes, recreate container
        ("closed", "error") => true,
        
        // From BUSY (actively processing)
        ("busy", "idle") => true,  // Processing complete
        ("busy", "error") => true,
        
        // Terminal states: error and deleted
        // These states have no outgoing transitions (can only be remixed)
        
        // Cannot transition to same state
        _ => false,
    }
}