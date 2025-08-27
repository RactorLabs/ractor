-- Build tasks for operator processing
-- Date: 2025-08-20

-- Build Tasks table for operator to process
CREATE TABLE IF NOT EXISTS build_tasks (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    task_type VARCHAR(50) NOT NULL DEFAULT 'space_build',
    space VARCHAR(255) NOT NULL,
    build_id VARCHAR(36) NOT NULL,
    payload JSON NOT NULL DEFAULT ('{}'),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    completed_at TIMESTAMP NULL,
    error TEXT,
    created_by VARCHAR(255) NOT NULL,
    
    INDEX idx_build_tasks_status (status),
    INDEX idx_build_tasks_space (space),
    INDEX idx_build_tasks_build_id (build_id),
    INDEX idx_build_tasks_created_at (created_at),
    
    CONSTRAINT fk_build_tasks_space FOREIGN KEY (space) REFERENCES spaces(name) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;