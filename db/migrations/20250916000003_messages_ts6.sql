-- Improve agent_messages.created_at precision to microseconds
ALTER TABLE agent_messages
  MODIFY COLUMN created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6);

-- Add a deterministic tie-breaker for listing order
-- (the API already orders by created_at; client code remains unchanged)
