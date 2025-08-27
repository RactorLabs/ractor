// Session state constants
#[allow(dead_code)]
pub const SESSION_STATE_INIT: &str = "init";
pub const SESSION_STATE_IDLE: &str = "idle";
#[allow(dead_code)]
pub const SESSION_STATE_BUSY: &str = "busy";
pub const SESSION_STATE_PAUSED: &str = "paused";
pub const SESSION_STATE_SUSPENDED: &str = "suspended";
pub const SESSION_STATE_ERROR: &str = "error";

// Message role constants  
#[allow(dead_code)]
pub const MESSAGE_ROLE_USER: &str = "user";
#[allow(dead_code)]
pub const MESSAGE_ROLE_AGENT: &str = "agent";
#[allow(dead_code)]
pub const MESSAGE_ROLE_SYSTEM: &str = "system";