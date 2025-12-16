-- Allow programmatic task types and widen column
ALTER TABLE sandbox_tasks
    DROP CHECK IF EXISTS sandbox_tasks_chk_2;

ALTER TABLE sandbox_tasks
    MODIFY task_type VARCHAR(20) NOT NULL DEFAULT 'NL';

ALTER TABLE sandbox_tasks
    ADD CONSTRAINT sandbox_tasks_chk_2 CHECK (task_type IN ('NL','SH','PY','JS','PROGRAMMATIC'));
