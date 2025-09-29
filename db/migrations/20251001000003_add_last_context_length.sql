ALTER TABLE agents
    ADD COLUMN last_context_length BIGINT NOT NULL DEFAULT 0;

-- backfill existing rows with 0 (default already handles existing)
UPDATE agents SET last_context_length = 0 WHERE last_context_length IS NULL;
