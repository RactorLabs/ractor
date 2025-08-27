pub fn can_transition_to(from: &str, to: &str) -> bool {
    match (from, to) {
        // From INIT
        ("init", "idle") => true,
        ("init", "error") => true,
        
        // From IDLE (ready and waiting)
        ("idle", "paused") => true,  // After timeout
        ("idle", "busy") => true,  // Processing request
        ("idle", "error") => true,
        
        // From PAUSED (container paused, waiting for reactivation)
        ("paused", "idle") => true,  // User returns, restart container
        ("paused", "error") => true,
        
        // From BUSY (actively processing)
        ("busy", "idle") => true,  // Processing complete
        ("busy", "error") => true,
        
        // From ERROR
        ("error", "init") => true,  // Reset
        ("error", "idle") => true,  // Recovery
        
        // Cannot transition to same state
        _ => false,
    }
}