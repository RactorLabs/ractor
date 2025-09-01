-- Add session publishing functionality
-- Date: 2025-09-01

-- Add published flag and publish settings to sessions table
ALTER TABLE sessions 
ADD COLUMN is_published BOOLEAN NOT NULL DEFAULT false,
ADD COLUMN published_at TIMESTAMP NULL,
ADD COLUMN published_by VARCHAR(255) NULL,
ADD COLUMN publish_permissions JSON DEFAULT ('{"data": true, "code": true, "secrets": true}');

-- Create index for published sessions
CREATE INDEX idx_sessions_published ON sessions(is_published, published_at);

-- Add constraint to ensure publish metadata is valid
ALTER TABLE sessions 
ADD CONSTRAINT sessions_publish_check 
CHECK (
    (is_published = false AND published_at IS NULL AND published_by IS NULL) OR
    (is_published = true AND published_at IS NOT NULL AND published_by IS NOT NULL)
);