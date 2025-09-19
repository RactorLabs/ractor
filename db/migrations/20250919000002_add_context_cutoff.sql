-- Add context cutoff marker for per-agent conversation trimming
ALTER TABLE agents
  ADD COLUMN context_cutoff_at TIMESTAMP NULL AFTER busy_from;

-- Optional index to help reads; harmless if not used
CREATE INDEX IF NOT EXISTS idx_agents_context_cutoff ON agents (context_cutoff_at);

