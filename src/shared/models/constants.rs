pub const SANDBOX_STATE_INITIALIZING: &str = "initializing";
pub const SANDBOX_STATE_IDLE: &str = "idle";
pub const SANDBOX_STATE_BUSY: &str = "busy";
pub const SANDBOX_STATE_TERMINATING: &str = "terminating";
pub const SANDBOX_STATE_TERMINATED: &str = "terminated";
pub const MAX_SANDBOX_FILE_BYTES: usize = 5 * 1024 * 1024; // 5MB safety cap for API uploads
