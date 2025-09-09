-- Raworc complete database schema with publishing and timeout features  
-- Date: 2025-09-02

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
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('Operator', 'User')),
    role_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, role_name),
    CONSTRAINT fk_role_bindings_role FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
    INDEX idx_role_bindings_principal (principal),
    INDEX idx_role_bindings_role_name (role_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Agents - Name-based architecture with publishing and timeout functionality
CREATE TABLE IF NOT EXISTS agents (
    name VARCHAR(64) PRIMARY KEY,
    created_by VARCHAR(255) NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'init',
    description TEXT NULL,
    parent_agent_name VARCHAR(64) NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMP NULL,
    metadata JSON DEFAULT ('{}'),
    tags JSON NOT NULL DEFAULT ('[]'),
    
    -- Publishing functionality
    is_published BOOLEAN NOT NULL DEFAULT false,
    published_at TIMESTAMP NULL,
    published_by VARCHAR(255) NULL,
    publish_permissions JSON DEFAULT ('{"code": true, "secrets": true, "content": true}'),
    
    -- Timeout functionality (idle/busy)
    idle_timeout_seconds INT NOT NULL DEFAULT 300,
    busy_timeout_seconds INT NOT NULL DEFAULT 900,
    idle_from TIMESTAMP NULL,
    busy_from TIMESTAMP NULL,
    
    -- Content HTTP server port mapping
    content_port INT NULL COMMENT 'Mapped host port for Content HTTP server (port 8000 inside container)',
    
    -- Constraints
    CONSTRAINT agents_name_check CHECK (name REGEXP '^[A-Za-z][A-Za-z0-9-]{0,61}[A-Za-z0-9]$'),
    CONSTRAINT agents_state_check CHECK (state IN ('init', 'idle', 'busy', 'slept')),
    CONSTRAINT agents_tags_check CHECK (JSON_TYPE(tags) = 'ARRAY'),
    CONSTRAINT agents_publish_check CHECK (
        (is_published = false AND published_at IS NULL AND published_by IS NULL) OR
        (is_published = true AND published_at IS NOT NULL AND published_by IS NOT NULL)
    ),
    CONSTRAINT agents_timeout_check CHECK (
        idle_timeout_seconds > 0 AND idle_timeout_seconds <= 604800 AND
        busy_timeout_seconds > 0 AND busy_timeout_seconds <= 604800
    ),
    CONSTRAINT fk_agents_parent FOREIGN KEY (parent_agent_name) REFERENCES agents(name) ON DELETE SET NULL,
    
    -- Indexes
    INDEX idx_agents_created_by (created_by),
    INDEX idx_agents_state (state),
    INDEX idx_agents_parent_agent_name (parent_agent_name),
    INDEX idx_agents_published (is_published, published_at),
    INDEX idx_agents_idle_from (idle_from, state),
    INDEX idx_agents_busy_from (busy_from, state),
    INDEX idx_agents_content_port (content_port)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Agent Messages
CREATE TABLE IF NOT EXISTS agent_messages (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    agent_name VARCHAR(64) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL CHECK (role IN ('user', 'agent', 'system')),
    content TEXT NOT NULL,
    metadata JSON DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_messages_agent FOREIGN KEY (agent_name) REFERENCES agents(name) ON DELETE CASCADE,
    INDEX idx_agent_messages_agent_name (agent_name),
    INDEX idx_agent_messages_created_by (created_by),
    INDEX idx_agent_messages_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Agent Tasks
CREATE TABLE IF NOT EXISTS agent_tasks (
    id CHAR(36) PRIMARY KEY DEFAULT (UUID()),
    task_type VARCHAR(50) NOT NULL,
    agent_name VARCHAR(64) NOT NULL,
    created_by VARCHAR(255) NOT NULL,
    payload JSON NOT NULL DEFAULT ('{}'),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    started_at TIMESTAMP NULL,
    completed_at TIMESTAMP NULL,
    error TEXT,
    -- Note: no FK to agents; tasks may reference agents scheduled for deletion
    INDEX idx_agent_tasks_status (status),
    INDEX idx_agent_tasks_agent_name (agent_name),
    INDEX idx_agent_tasks_created_by (created_by),
    INDEX idx_agent_tasks_created_at (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Seed operators (admin, demo)
INSERT IGNORE INTO operators (name, password_hash, description, active) 
VALUES (
    'admin',
    '$2b$12$dZTOY/3oZQxB10jUMElLZ.NrH8JpUpuVGKxtnnRR7lnVXJF92QkI2',
    'Default admin operator',
    true
);

-- Demo operator (password: demo)
INSERT IGNORE INTO operators (name, password_hash, description, active)
VALUES (
    'demo',
    '$2b$12$mZj.Uuy1CkHbLgoO0IO2ouZW7B8N3bx1GxPtogd3YzzefsfkYP7NW',
    'Demo user',
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

-- User role: access to agents only; no operator management
INSERT IGNORE INTO roles (name, description, rules) VALUES
(
    'user',
    'Standard user role with access to agents; no operator management',
    JSON_ARRAY(
        -- Allow full access to agents endpoints
        JSON_OBJECT(
            'api_groups', JSON_ARRAY('api'),
            'resources', JSON_ARRAY('agents'),
            'verbs', JSON_ARRAY('*')
        )
    )
);

-- Role bindings
INSERT IGNORE INTO role_bindings (principal, principal_type, role_name) 
VALUES 
    ('admin', 'Operator', 'admin'),
    ('demo',  'Operator', 'user');
