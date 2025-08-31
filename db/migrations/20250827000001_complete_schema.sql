-- Raworc simplified database schema
-- Date: 2025-08-31

-- Service Accounts
CREATE TABLE IF NOT EXISTS service_accounts (
    name VARCHAR(255) PRIMARY KEY,
    password_hash TEXT NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    last_login_at TIMESTAMP NULL,
    CONSTRAINT service_accounts_name_check CHECK (name REGEXP '^[a-zA-Z0-9_.-]+$'),
    INDEX idx_service_accounts_active (active)
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
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('ServiceAccount', 'User')),
    role_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, role_name),
    CONSTRAINT fk_role_bindings_role FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
    INDEX idx_role_bindings_principal (principal),
    INDEX idx_role_bindings_role_name (role_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sessions
CREATE TABLE IF NOT EXISTS sessions (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    created_by VARCHAR(255) NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'init',
    container_id VARCHAR(255),
    persistent_volume_id VARCHAR(255),
    parent_session_id CHAR(36),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMP NULL,
    metadata JSON DEFAULT ('{}'),
    CONSTRAINT sessions_state_check CHECK (state IN ('init', 'idle', 'busy', 'closed', 'errored', 'deleted')),
    CONSTRAINT fk_sessions_parent FOREIGN KEY (parent_session_id) REFERENCES sessions(id) ON DELETE SET NULL,
    INDEX idx_sessions_created_by (created_by),
    INDEX idx_sessions_state (state),
    INDEX idx_sessions_parent_session_id (parent_session_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Session Messages
CREATE TABLE IF NOT EXISTS session_messages (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    session_id CHAR(36) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL CHECK (role IN ('user', 'agent', 'system')),
    content TEXT NOT NULL,
    metadata JSON DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_messages_session FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    INDEX idx_session_messages_session_id (session_id),
    INDEX idx_session_messages_created_by (created_by),
    INDEX idx_session_messages_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Session Tasks
CREATE TABLE IF NOT EXISTS session_tasks (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    task_type VARCHAR(50) NOT NULL,
    session_id CHAR(36) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    payload JSON NOT NULL DEFAULT ('{}'),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    completed_at TIMESTAMP NULL,
    error TEXT,
    CONSTRAINT fk_tasks_session FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    INDEX idx_session_tasks_status (status),
    INDEX idx_session_tasks_session_id (session_id),
    INDEX idx_session_tasks_created_by (created_by),
    INDEX idx_session_tasks_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Default admin service account (password: admin)
INSERT IGNORE INTO service_accounts (name, password_hash, description, active) 
VALUES (
    'admin',
    '$2b$12$xJxdkbovt0jOPDz54RrAeufRUuWRCEJRhClksgUmN9uKKUbG.I8Ly',
    'Default admin account',
    true
);

-- Default roles
INSERT IGNORE INTO roles (name, description, rules) VALUES
(
    'admin',
    'Full administrative access',
    JSON_ARRAY(
        JSON_OBJECT(
            'api_groups', JSON_ARRAY('*'),
            'resources', JSON_ARRAY('*'),
            'verbs', JSON_ARRAY('*')
        )
    )
),
(
    'user',
    'Can manage own sessions',
    JSON_ARRAY(
        JSON_OBJECT(
            'api_groups', JSON_ARRAY('api'),
            'resources', JSON_ARRAY('sessions', 'session_messages', 'session_tasks'),
            'verbs', JSON_ARRAY('create', 'read', 'update', 'delete', 'list', 'get')
        )
    )
);

-- Admin role binding
INSERT IGNORE INTO role_bindings (principal, principal_type, role_name) 
VALUES (
    'admin',
    'ServiceAccount',
    'admin'
);