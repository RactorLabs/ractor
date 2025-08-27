-- Simplify session lifecycle to suspend/delete only
-- Remove termination and pause concepts

-- First update any existing sessions to new state names
UPDATE sessions SET state = 'closed' WHERE state = 'paused';
UPDATE sessions SET state = 'closed' WHERE state = 'suspended';

-- Update session state constraint with new simplified states
ALTER TABLE sessions 
    DROP CONSTRAINT sessions_state_check,
    ADD CONSTRAINT sessions_state_check CHECK (state IN ('init', 'idle', 'busy', 'closed', 'error', 'deleted'));

-- Remove unnecessary columns (using state='deleted' instead of deleted_at)
ALTER TABLE sessions 
    DROP COLUMN terminated_at,
    DROP COLUMN termination_reason,
    DROP COLUMN deleted_at,
    DROP COLUMN started_at;