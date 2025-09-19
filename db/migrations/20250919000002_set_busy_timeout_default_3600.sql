-- Set default busy timeout to 3600 seconds (1 hour)
ALTER TABLE agents
  MODIFY busy_timeout_seconds INT NOT NULL DEFAULT 3600;

