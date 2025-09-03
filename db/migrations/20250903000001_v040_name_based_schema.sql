-- v0.4.0 Migration: Complete overhaul to name-based architecture
-- Date: 2025-09-03
-- BREAKING CHANGE: No backward compatibility - complete data reset required

-- Drop all existing tables (complete reset)
DROP TABLE IF EXISTS session_tasks;
DROP TABLE IF EXISTS session_messages; 
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS role_bindings;
DROP TABLE IF EXISTS roles;
DROP TABLE IF EXISTS operators;

-- Operators (unchanged)
CREATE TABLE operators (
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

-- Roles (unchanged)
CREATE TABLE roles (
    name VARCHAR(255) PRIMARY KEY,
    rules JSON NOT NULL DEFAULT ('[]'),
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Role Bindings (unchanged)
CREATE TABLE role_bindings (
    principal VARCHAR(255) NOT NULL,
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('Operator', 'User')),
    role_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, role_name),
    CONSTRAINT fk_role_bindings_role FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
    INDEX idx_role_bindings_principal (principal),
    INDEX idx_role_bindings_role_name (role_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Sessions - MAJOR CHANGES: name as primary key, removed IDs and data folder references
CREATE TABLE sessions (
    name VARCHAR(64) PRIMARY KEY,  -- Primary key, alphanumeric + hyphens, max 64 chars
    created_by VARCHAR(255) NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'init',
    parent_session_name VARCHAR(64) NULL,  -- Changed from parent_session_id
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMP NULL,
    metadata JSON DEFAULT ('{}'),
    
    -- Publishing functionality
    is_published BOOLEAN NOT NULL DEFAULT false,
    published_at TIMESTAMP NULL,
    published_by VARCHAR(255) NULL,
    publish_permissions JSON DEFAULT ('{"code": true, "secrets": true, "canvas": true}'),  -- Removed data, added canvas
    
    -- Timeout functionality  
    timeout_seconds INT NOT NULL DEFAULT 300,
    auto_close_at TIMESTAMP NULL,
    
    -- Canvas HTTP server port mapping (public server on port 8000)
    canvas_port INT NULL COMMENT 'Mapped host port for Canvas HTTP server (port 8000 inside container)',
    
    -- Constraints
    CONSTRAINT sessions_name_check CHECK (name REGEXP '^[a-z][a-z0-9-]{0,61}[a-z0-9]$'),  -- Strict name validation
    CONSTRAINT sessions_state_check CHECK (state IN ('init', 'idle', 'busy', 'closed', 'errored', 'deleted')),
    CONSTRAINT sessions_publish_check CHECK (
        (is_published = false AND published_at IS NULL AND published_by IS NULL) OR
        (is_published = true AND published_at IS NOT NULL AND published_by IS NOT NULL)
    ),
    CONSTRAINT sessions_timeout_check CHECK (timeout_seconds > 0 AND timeout_seconds <= 604800),
    CONSTRAINT fk_sessions_parent FOREIGN KEY (parent_session_name) REFERENCES sessions(name) ON DELETE SET NULL,
    
    -- Indexes
    INDEX idx_sessions_created_by (created_by),
    INDEX idx_sessions_state (state),
    INDEX idx_sessions_parent_session_name (parent_session_name),
    INDEX idx_sessions_published (is_published, published_at),
    INDEX idx_sessions_auto_close (auto_close_at, state),
    INDEX idx_sessions_canvas_port (canvas_port)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Session Messages - Updated to use session name instead of ID
CREATE TABLE session_messages (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),  -- Keep message IDs for ordering
    session_name VARCHAR(64) NOT NULL,         -- Changed from session_id
    created_by VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL CHECK (role IN ('user', 'host', 'system')),
    content TEXT NOT NULL,
    metadata JSON DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_messages_session FOREIGN KEY (session_name) REFERENCES sessions(name) ON DELETE CASCADE,
    INDEX idx_session_messages_session_name (session_name),
    INDEX idx_session_messages_created_by (created_by),
    INDEX idx_session_messages_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Session Tasks - Updated to use session name instead of ID
CREATE TABLE session_tasks (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),  -- Keep task IDs for ordering
    task_type VARCHAR(50) NOT NULL,
    session_name VARCHAR(64) NOT NULL,         -- Changed from session_id
    created_by VARCHAR(255) NOT NULL,
    payload JSON NOT NULL DEFAULT ('{}'),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    completed_at TIMESTAMP NULL,
    error TEXT,
    CONSTRAINT fk_tasks_session FOREIGN KEY (session_name) REFERENCES sessions(name) ON DELETE CASCADE,
    INDEX idx_session_tasks_status (status),
    INDEX idx_session_tasks_session_name (session_name),
    INDEX idx_session_tasks_created_by (created_by),
    INDEX idx_session_tasks_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Default admin operator (password: admin)
INSERT INTO operators (name, password_hash, description, active) 
VALUES (
    'admin',
    '$2b$12$xJxdkbovt0jOPDz54RrAeufRUuWRCEJRhClksgUmN9uKKUbG.I8Ly',
    'Default admin operator',
    true
);

-- Default roles
INSERT INTO roles (name, description, rules) VALUES
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
INSERT INTO role_bindings (principal, principal_type, role_name) 
VALUES 
(
    'admin',
    'Operator',
    'admin'
);