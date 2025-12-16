-- MCP tool usage examples for teaching agents how to call tools
CREATE TABLE IF NOT EXISTS mcp_tool_examples (
    id CHAR(36) NOT NULL PRIMARY KEY,
    tool_id CHAR(36) NOT NULL,
    title VARCHAR(255) NULL,
    body JSON NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_mcp_tool_examples_tool FOREIGN KEY (tool_id) REFERENCES mcp_tools(id) ON DELETE CASCADE,
    INDEX idx_mcp_tool_examples_tool (tool_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
