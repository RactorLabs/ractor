// Session state constants
#[allow(dead_code)]
pub const SESSION_STATE_INIT: &str = "init";
pub const SESSION_STATE_IDLE: &str = "idle";
#[allow(dead_code)]
pub const SESSION_STATE_BUSY: &str = "busy";
pub const SESSION_STATE_CLOSED: &str = "closed";
pub const SESSION_STATE_ERROR: &str = "error";
#[allow(dead_code)]
pub const SESSION_STATE_DELETED: &str = "deleted";

// Message role constants  
#[allow(dead_code)]
pub const MESSAGE_ROLE_USER: &str = "user";
#[allow(dead_code)]
pub const MESSAGE_ROLE_AGENT: &str = "agent";
#[allow(dead_code)]
pub const MESSAGE_ROLE_SYSTEM: &str = "system";