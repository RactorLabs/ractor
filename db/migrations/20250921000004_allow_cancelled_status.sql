-- Allow 'cancelled' as a valid status for agent_responses
-- Drop the existing CHECK constraint and recreate it with the new set

ALTER TABLE agent_responses
  DROP CHECK agent_responses_chk_1;

ALTER TABLE agent_responses
  ADD CONSTRAINT agent_responses_chk_1 CHECK (status IN ('pending','processing','completed','failed','cancelled'));

