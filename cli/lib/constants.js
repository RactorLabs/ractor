// Session state constants
const SESSION_STATE_INIT = 'init';
const SESSION_STATE_IDLE = 'idle';
const SESSION_STATE_BUSY = 'busy';
const SESSION_STATE_CLOSED = 'closed';
const SESSION_STATE_ERRORED = 'errored';
const SESSION_STATE_DELETED = 'deleted';

// Message role constants
const MESSAGE_ROLE_USER = 'user';
const MESSAGE_ROLE_AGENT = 'agent';
const MESSAGE_ROLE_SYSTEM = 'system';

module.exports = {
  // Session states
  SESSION_STATE_INIT,
  SESSION_STATE_IDLE,
  SESSION_STATE_BUSY,
  SESSION_STATE_CLOSED,
  SESSION_STATE_ERRORED,
  SESSION_STATE_DELETED,
  
  // Message roles
  MESSAGE_ROLE_USER,
  MESSAGE_ROLE_AGENT,
  MESSAGE_ROLE_SYSTEM
};