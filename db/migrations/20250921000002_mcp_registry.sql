-- MCP registry schema
CREATE TABLE IF NOT EXISTS mcp_servers (
    id CHAR(36) NOT NULL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    base_url TEXT NOT NULL,
    auth_type VARCHAR(32) NULL,
    auth_payload JSON NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'unknown',
    last_seen_at DATETIME NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS mcp_tools (
    id CHAR(36) NOT NULL PRIMARY KEY,
    server_id CHAR(36) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT NULL,
    input_schema JSON NULL,
    output_schema JSON NULL,
    metadata JSON NULL,
    version VARCHAR(64) NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uniq_mcp_tool (server_id, name),
    CONSTRAINT fk_mcp_tools_server FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS mcp_invocations (
    id CHAR(36) NOT NULL PRIMARY KEY,
    server_id CHAR(36) NOT NULL,
    tool_name VARCHAR(255) NOT NULL,
    sandbox_id CHAR(36) NULL,
    request JSON NULL,
    response JSON NULL,
    status VARCHAR(32) NOT NULL,
    error_text TEXT NULL,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at DATETIME NULL,
    INDEX idx_mcp_invocations_server_tool (server_id, tool_name),
    CONSTRAINT fk_mcp_invocations_server FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
