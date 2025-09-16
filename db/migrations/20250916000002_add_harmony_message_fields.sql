-- Add Harmony protocol fields to agent_messages
ALTER TABLE agent_messages
  ADD COLUMN author_name VARCHAR(255) NULL AFTER created_by,
  ADD COLUMN recipient VARCHAR(255) NULL AFTER role,
  ADD COLUMN channel VARCHAR(64) NULL AFTER recipient,
  ADD COLUMN content_type VARCHAR(64) NULL AFTER channel,
  ADD COLUMN content_json JSON NULL AFTER content;

-- Optional indexes for querying by channel/recipient
CREATE INDEX IF NOT EXISTS idx_agent_messages_channel ON agent_messages (channel);
CREATE INDEX IF NOT EXISTS idx_agent_messages_recipient ON agent_messages (recipient);
