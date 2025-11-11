

-- Operators
CREATE TABLE IF NOT EXISTS operators (
    name VARCHAR(255) PRIMARY KEY,
    password_hash TEXT NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    last_login_at TIMESTAMP NULL,
    CONSTRAINT operators_name_check CHECK (name REGEXP '^[a-zA-Z0-9_.-]+$'),
    INDEX idx_operators_active (active)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Roles
CREATE TABLE IF NOT EXISTS roles (
    name VARCHAR(255) PRIMARY KEY,
    rules JSON NOT NULL DEFAULT ('[]'),
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Role Bindings
CREATE TABLE IF NOT EXISTS role_bindings (
    principal VARCHAR(255) NOT NULL,
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('Admin', 'User')),
    role_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, role_name),
    CONSTRAINT fk_role_bindings_role FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
    INDEX idx_role_bindings_principal (principal),
    INDEX idx_role_bindings_role_name (role_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sandboxes - UUID-based architecture with timeout functionality
CREATE TABLE IF NOT EXISTS sandboxes (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    created_by VARCHAR(255) NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'initializing',
    description TEXT NULL,
    snapshot_id CHAR(36) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMP NULL,
    metadata JSON DEFAULT ('{}'),
    tags JSON NOT NULL DEFAULT ('[]'),

    -- Timeout functionality (idle/busy)
    idle_timeout_seconds INT NOT NULL DEFAULT 900,
    idle_from TIMESTAMP NULL,
    busy_from TIMESTAMP NULL,

    -- Context cutoff marker for conversation trimming
    context_cutoff_at TIMESTAMP NULL,
    last_context_length BIGINT NOT NULL DEFAULT 0,

    -- Constraints
    CONSTRAINT sandboxes_state_check CHECK (state IN ('initializing', 'idle', 'busy', 'terminating', 'terminated')),
    CONSTRAINT sandboxes_tags_check CHECK (JSON_TYPE(tags) = 'ARRAY'),
    CONSTRAINT sandboxes_timeout_check CHECK (
        idle_timeout_seconds > 0 AND idle_timeout_seconds <= 604800
    ),

    -- Indexes
    INDEX idx_sandboxes_created_by (created_by),
    INDEX idx_sandboxes_state (state),
    INDEX idx_sandboxes_idle_from (idle_from, state),
    INDEX idx_sandboxes_busy_from (busy_from, state),
    INDEX idx_sandboxes_context_cutoff (context_cutoff_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sandbox Tasks (user conversations)
CREATE TABLE IF NOT EXISTS sandbox_tasks (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    sandbox_id CHAR(36) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processing','completed','failed','cancelled')),
    input JSON NOT NULL,
    output JSON NOT NULL,
    steps JSON NOT NULL DEFAULT ('[]'),
    timeout_seconds INT NULL,
    timeout_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_tasks_sandbox FOREIGN KEY (sandbox_id) REFERENCES sandboxes(id) ON DELETE CASCADE,
    INDEX idx_sandbox_tasks_sandbox_id (sandbox_id),
    INDEX idx_sandbox_tasks_created_by (created_by),
    INDEX idx_sandbox_tasks_created_at (created_at),
    INDEX idx_sandbox_tasks_timeout_at (timeout_at),
    INDEX idx_sandbox_tasks_sandbox_created_at_id (sandbox_id, created_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sandbox Requests
CREATE TABLE IF NOT EXISTS sandbox_requests (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    request_type VARCHAR(50) NOT NULL,
    sandbox_id CHAR(36) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    payload JSON NOT NULL DEFAULT ('{}'),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    completed_at TIMESTAMP NULL,
    error TEXT,
    INDEX idx_sandbox_requests_status (status),
    INDEX idx_sandbox_requests_sandbox_id (sandbox_id),
    INDEX idx_sandbox_requests_created_by (created_by),
    INDEX idx_sandbox_requests_created_at (created_at),
    INDEX idx_sandbox_requests_status_created_at (status, created_at),
    INDEX idx_sandbox_requests_sandbox_type_status_created_at (sandbox_id, request_type, status, created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Snapshots
CREATE TABLE IF NOT EXISTS snapshots (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    sandbox_id CHAR(36) NOT NULL,
    trigger_type ENUM('manual', 'termination') NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    metadata JSON DEFAULT ('{}'),
    CONSTRAINT fk_snapshots_sandbox FOREIGN KEY (sandbox_id) REFERENCES sandboxes(id) ON DELETE CASCADE,
    INDEX idx_snapshots_sandbox_id (sandbox_id),
    INDEX idx_snapshots_trigger_type (trigger_type),
    INDEX idx_snapshots_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Default admin operator
INSERT IGNORE INTO operators (name, password_hash, description, active)
VALUES (
    'admin',
    '$2b$12$xJxdkbovt0jOPDz54RrAeufRUuWRCEJRhClksgUmN9uKKUbG.I8Ly',
    'Default admin operator',
    true
);


-- Default roles
INSERT IGNORE INTO roles (name, description, rules) VALUES
(
    'admin',
    'Full administrative access including operator management',
    JSON_ARRAY(
        JSON_OBJECT(
            'api_groups', JSON_ARRAY('*'),
            'resources', JSON_ARRAY('*'),
            'verbs', JSON_ARRAY('*')
        )
    )
);


-- Role bindings
INSERT IGNORE INTO role_bindings (principal, principal_type, role_name)
VALUES
    ('admin', 'Admin', 'admin');


-- Blocked Principals
CREATE TABLE IF NOT EXISTS blocked_principals (
    principal VARCHAR(255) NOT NULL,
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('Admin', 'User')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, principal_type),
    INDEX idx_blocked_principals_principal (principal),
    INDEX idx_blocked_principals_type (principal_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
