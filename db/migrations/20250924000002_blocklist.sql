-- Blocked Principals Table
-- Stores principals (users/admins) that are blocked from accessing protected APIs.
-- Note: Public endpoints remain accessible without auth.

CREATE TABLE IF NOT EXISTS blocked_principals (
    principal VARCHAR(255) NOT NULL,
    principal_type VARCHAR(50) NOT NULL CHECK (principal_type IN ('Admin', 'User')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (principal, principal_type),
    INDEX idx_blocked_principals_principal (principal),
    INDEX idx_blocked_principals_type (principal_type)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

