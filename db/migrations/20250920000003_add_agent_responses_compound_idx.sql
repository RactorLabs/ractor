-- Improve performance of agent_responses scans and avoid filesort on common queries
-- Adds a composite index to support filtering by agent_name with ordering by created_at and tie-break by id
ALTER TABLE agent_responses
  ADD INDEX idx_agent_responses_agent_created_at_id (agent_name, created_at, id);

