// Agent state constants
const AGENT_STATE_INIT = 'init';
const AGENT_STATE_IDLE = 'idle';
const AGENT_STATE_BUSY = 'busy';
const AGENT_STATE_SLEPT = 'slept';
const AGENT_STATE_ERRORED = 'errored';
const AGENT_STATE_DELETED = 'deleted';

// Message role constants
const MESSAGE_ROLE_USER = 'user';
const MESSAGE_ROLE_AGENT = 'agent';
const MESSAGE_ROLE_SYSTEM = 'system';

module.exports = {
  // Agent states
  AGENT_STATE_INIT,
  AGENT_STATE_IDLE,
  AGENT_STATE_BUSY,
  AGENT_STATE_SLEPT,
  AGENT_STATE_ERRORED,
  AGENT_STATE_DELETED,

  // Message roles
  MESSAGE_ROLE_USER,
  MESSAGE_ROLE_AGENT,
  MESSAGE_ROLE_SYSTEM
};
