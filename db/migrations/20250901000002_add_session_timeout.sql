-- Add session timeout functionality
-- Date: 2025-09-01

-- Add timeout fields to sessions table
ALTER TABLE sessions 
ADD COLUMN timeout_seconds INT NOT NULL DEFAULT 60,
ADD COLUMN auto_close_at TIMESTAMP NULL;

-- Create index for auto-close monitoring
CREATE INDEX idx_sessions_auto_close ON sessions(auto_close_at, state);

-- Add constraint for valid timeout values
ALTER TABLE sessions 
ADD CONSTRAINT sessions_timeout_check 
CHECK (timeout_seconds > 0 AND timeout_seconds <= 604800); -- Max 1 week (7 days * 24 hours * 60 minutes * 60 seconds)

-- Update auto_close_at for existing active sessions based on last_activity_at + timeout
UPDATE sessions 
SET auto_close_at = DATE_ADD(COALESCE(last_activity_at, created_at), INTERVAL timeout_seconds SECOND)
WHERE state IN ('init', 'idle', 'busy') AND auto_close_at IS NULL;