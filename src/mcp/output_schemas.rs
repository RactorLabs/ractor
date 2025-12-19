use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::mcp::models::McpToolDescriptor;

type SchemaMap = HashMap<String, HashMap<String, Value>>;

/// Static output schemas for MCP servers whose tool metadata does not include them.
pub static OUTPUT_SCHEMA_OVERRIDES: Lazy<SchemaMap> = Lazy::new(|| {
    let mut servers: SchemaMap = HashMap::new();

    let mut github = HashMap::new();
    github.insert(
        "search_repositories".to_string(),
        json!({
            "type": "object",
            "properties": {
                "total_count": { "type": "integer" },
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "name": { "type": "string" },
                            "full_name": { "type": "string" },
                            "owner": {
                                "type": "object",
                                "properties": {
                                    "login": { "type": "string" },
                                    "id": { "type": "integer" },
                                    "html_url": { "type": "string" },
                                    "avatar_url": { "type": "string" }
                                }
                            },
                            "private": { "type": "boolean" },
                            "html_url": { "type": "string" },
                            "description": { "type": ["string", "null"] },
                            "language": { "type": ["string", "null"] },
                            "topics": { "type": "array", "items": { "type": "string" } },
                            "visibility": { "type": "string" },
                            "archived": { "type": "boolean" },
                            "forks_count": { "type": "integer" },
                            "stargazers_count": { "type": "integer" },
                            "watchers_count": { "type": "integer" },
                            "open_issues_count": { "type": "integer" },
                            "default_branch": { "type": "string" },
                            "updated_at": { "type": "string", "description": "ISO 8601 timestamp" }
                        },
                        "required": ["id", "name", "full_name", "html_url"]
                    }
                },
                "next_page": { "type": ["integer", "null"] },
                "next": { "type": ["string", "null"] },
                "next_token": { "type": ["string", "null"] }
            },
            "required": ["items"]
        }),
    );
    github.insert(
        "search_issues".to_string(),
        json!({
            "type": "object",
            "properties": {
                "total_count": { "type": "integer" },
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "number": { "type": "integer" },
                            "title": { "type": "string" },
                            "state": { "type": "string" },
                            "html_url": { "type": "string" },
                            "body": { "type": ["string", "null"] },
                            "user": {
                                "type": "object",
                                "properties": {
                                    "login": { "type": "string" },
                                    "html_url": { "type": "string" }
                                }
                            },
                            "assignee": {
                                "type": "object",
                                "properties": {
                                    "login": { "type": "string" },
                                    "html_url": { "type": "string" }
                                }
                            },
                            "assignees": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "login": { "type": "string" },
                                        "html_url": { "type": "string" }
                                    }
                                }
                            },
                            "labels": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" },
                                        "color": { "type": "string" }
                                    }
                                }
                            },
                            "comments": { "type": "integer" },
                            "pull_request": {
                                "type": "object",
                                "properties": { "url": { "type": "string" } }
                            },
                            "repository_url": { "type": "string" },
                            "repository": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "full_name": { "type": "string" }
                                }
                            },
                            "created_at": { "type": "string" },
                            "updated_at": { "type": "string" },
                            "closed_at": { "type": ["string", "null"] }
                        },
                        "required": ["id", "number", "title", "state", "html_url"]
                    }
                },
                "next_page": { "type": ["integer", "null"] },
                "next": { "type": ["string", "null"] },
                "next_token": { "type": ["string", "null"] }
            },
            "required": ["items"]
        }),
    );
    github.insert(
        "search_pull_requests".to_string(),
        json!({
            "type": "object",
            "properties": {
                "total_count": { "type": "integer" },
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "number": { "type": "integer" },
                            "title": { "type": "string" },
                            "state": { "type": "string" },
                            "html_url": { "type": "string" },
                            "body": { "type": ["string", "null"] },
                            "user": {
                                "type": "object",
                                "properties": {
                                    "login": { "type": "string" },
                                    "html_url": { "type": "string" }
                                }
                            },
                            "head": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" },
                                    "sha": { "type": "string" },
                                    "repo": {
                                        "type": "object",
                                        "properties": {
                                            "full_name": { "type": "string" },
                                            "html_url": { "type": "string" }
                                        }
                                    }
                                }
                            },
                            "base": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" },
                                    "sha": { "type": "string" },
                                    "repo": {
                                        "type": "object",
                                        "properties": {
                                            "full_name": { "type": "string" },
                                            "html_url": { "type": "string" }
                                        }
                                    }
                                }
                            },
                            "draft": { "type": "boolean" },
                            "created_at": { "type": "string" },
                            "updated_at": { "type": "string" },
                            "merged_at": { "type": ["string", "null"] }
                        },
                        "required": ["id", "number", "title", "state", "html_url"]
                    }
                },
                "next_page": { "type": ["integer", "null"] },
                "next": { "type": ["string", "null"] },
                "next_token": { "type": ["string", "null"] }
            },
            "required": ["items"]
        }),
    );
    github.insert(
        "issue_read".to_string(),
        json!({
            "type": "object",
            "properties": {
                "issue": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "number": { "type": "integer" },
                        "title": { "type": "string" },
                        "state": { "type": "string" },
                        "html_url": { "type": "string" },
                        "body": { "type": ["string", "null"] },
                        "user": {
                            "type": "object",
                            "properties": {
                                "login": { "type": "string" },
                                "html_url": { "type": "string" }
                            }
                        },
                        "assignee": {
                            "type": "object",
                            "properties": {
                                "login": { "type": "string" },
                                "html_url": { "type": "string" }
                            }
                        },
                        "labels": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "color": { "type": "string" }
                                }
                            }
                        },
                        "comments": { "type": "integer" },
                        "created_at": { "type": "string" },
                        "updated_at": { "type": "string" },
                        "closed_at": { "type": ["string", "null"] }
                    },
                    "required": ["id", "number", "title", "state", "html_url"]
                },
                "comments": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "user": {
                                "type": "object",
                                "properties": {
                                    "login": { "type": "string" },
                                    "html_url": { "type": "string" }
                                }
                            },
                            "body": { "type": "string" },
                            "created_at": { "type": "string" },
                            "updated_at": { "type": "string" }
                        }
                    }
                }
            },
            "required": ["issue"]
        }),
    );
    github.insert(
        "get_me".to_string(),
        json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "login": { "type": "string" },
                        "name": { "type": ["string", "null"] },
                        "email": { "type": ["string", "null"] },
                        "html_url": { "type": "string" },
                        "avatar_url": { "type": "string" },
                        "company": { "type": ["string", "null"] },
                        "location": { "type": ["string", "null"] },
                        "bio": { "type": ["string", "null"] },
                        "created_at": { "type": "string" },
                        "plan": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "space": { "type": "integer" },
                                "private_repos": { "type": "integer" },
                                "collaborators": { "type": "integer" }
                            }
                        }
                    },
                    "required": ["id", "login"]
                }
            },
            "required": ["user"]
        }),
    );

    servers.insert("github".to_string(), github);

    let mut hubspot = HashMap::new();
    hubspot.insert(
        "search_deals".to_string(),
        json!({
            "type": "object",
            "properties": {
                "results": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "properties": { "type": "object", "additionalProperties": true },
                            "associations": { "type": "object" },
                            "archived": { "type": "boolean" },
                            "createdAt": { "type": "string" },
                            "updatedAt": { "type": "string" }
                        },
                        "required": ["id"]
                    }
                },
                "paging": {
                    "type": "object",
                    "properties": {
                        "next": {
                            "type": "object",
                            "properties": {
                                "after": { "type": "string" }
                            },
                            "required": ["after"]
                        }
                    }
                }
            },
            "required": ["results"]
        }),
    );

    servers.insert("hubspot".to_string(), hubspot);
    servers
});

fn generic_object_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

pub fn ensure_output_schema(server_name: &str, tool_name: &str, output_schema: &mut Option<Value>) {
    if output_schema.is_some() {
        return;
    }
    let server_key = server_name.to_lowercase();
    let key = tool_name.to_lowercase();
    if let Some(schema) = OUTPUT_SCHEMA_OVERRIDES
        .get(&server_key)
        .and_then(|m| m.get(&key))
    {
        *output_schema = Some(schema.clone());
    } else {
        *output_schema = Some(generic_object_schema());
    }
}

pub fn apply_output_schema_overrides(server_name: &str, tools: &mut [McpToolDescriptor]) {
    for tool in tools.iter_mut() {
        ensure_output_schema(server_name, &tool.name, &mut tool.output_schema);
    }
}
