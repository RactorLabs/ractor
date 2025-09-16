use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::ollama::{ToolDef, ToolFunction, ToolType};

/// Core trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the tool parameters as JSON schema
    fn parameters(&self) -> serde_json::Value;

    /// Execute the tool with given arguments
    async fn execute(&self, args: &serde_json::Value) -> Result<String>;
}

/// Maps parameters from one tool format to another (for aliases)
pub trait ParameterMapper: Send + Sync {
    fn map(&self, args: &serde_json::Value) -> serde_json::Value;
}

/// Configuration for tool aliases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAlias {
    pub name: String,
    pub alias_for: String,
    pub parameter_mapping: Option<HashMap<String, String>>,
}

/// Configuration for dynamic tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub tools: Vec<ToolAlias>,
}

/// Registry that manages all available tools and their aliases
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>>,
    aliases: Arc<RwLock<HashMap<String, String>>>, // alias -> canonical_name
    mappers: Arc<RwLock<HashMap<String, Box<dyn ParameterMapper>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            aliases: Arc::new(RwLock::new(HashMap::new())),
            mappers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool with the registry
    pub async fn register_tool(&self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        info!("Registering tool: {}", name);
        let mut tools = self.tools.write().await;
        tools.insert(name, tool);
    }

    /// Register an alias for an existing tool
    pub async fn register_alias(
        &self,
        alias: &str,
        target: &str,
        mapper: Option<Box<dyn ParameterMapper>>,
    ) {
        info!("Registering alias '{}' -> '{}'", alias, target);

        let mut aliases = self.aliases.write().await;
        aliases.insert(alias.to_string(), target.to_string());

        if let Some(mapper) = mapper {
            let mut mappers = self.mappers.write().await;
            mappers.insert(alias.to_string(), mapper);
        }
    }

    /// Get a tool by name, checking aliases first
    pub async fn get_tool(&self, name: &str) -> Option<String> {
        // Check if it's an alias first
        let aliases = self.aliases.read().await;
        if let Some(canonical_name) = aliases.get(name) {
            return Some(canonical_name.clone());
        }

        // Check if it's a direct tool name
        let tools = self.tools.read().await;
        if tools.contains_key(name) {
            return Some(name.to_string());
        }

        None
    }

    /// Execute a tool with the given arguments
    pub async fn execute_tool(&self, name: &str, args: &serde_json::Value) -> Result<String> {
        // No special stripping of non-standard formats; use tool name as-is
        let clean_name = name;
        tracing::info!("Executing tool: '{}'", clean_name);
        
        // Map parameters if it's an alias
        let (canonical_name, mapped_args) = {
            let aliases = self.aliases.read().await;
            if let Some(canonical_name) = aliases.get(clean_name) {
                let mappers = self.mappers.read().await;
                let mapped_args = if let Some(mapper) = mappers.get(clean_name) {
                    mapper.map(args)
                } else {
                    args.clone()
                };
                (canonical_name.clone(), mapped_args)
            } else {
                (clean_name.to_string(), args.clone())
            }
        };

        // Execute the tool
        let tools = self.tools.read().await;
        if let Some(tool) = tools.get(&canonical_name) {
            tool.execute(&mapped_args).await
        } else {
            anyhow::bail!("Tool '{}' not found", canonical_name);
        }
    }

    /// Generate Ollama-compatible tool definitions
    pub async fn generate_ollama_tools(&self) -> Vec<ToolDef> {
        let mut tool_defs = Vec::new();

        // Add all registered tools
        let tools = self.tools.read().await;
        for (name, tool) in tools.iter() {
            tool_defs.push(ToolDef {
                typ: ToolType::Function,
                function: ToolFunction {
                    name: name.clone(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters(),
                },
            });
        }

        // Add aliases as separate tools
        let aliases = self.aliases.read().await;
        for (alias_name, canonical_name) in aliases.iter() {
            if let Some(tool) = tools.get(canonical_name) {
                tool_defs.push(ToolDef {
                    typ: ToolType::Function,
                    function: ToolFunction {
                        name: alias_name.clone(),
                        description: format!(
                            "{} (alias for {})",
                            tool.description(),
                            canonical_name
                        ),
                        parameters: tool.parameters(),
                    },
                });
            }
        }

        info!("Generated {} tool definitions for Ollama", tool_defs.len());
        tool_defs
    }

    /// Load tool configuration from file
    pub async fn load_config(&self, config_path: &Path) -> Result<()> {
        if !config_path.exists() {
            info!(
                "No tool config found at {}, skipping",
                config_path.display()
            );
            return Ok(());
        }

        let config_str = tokio::fs::read_to_string(config_path).await?;
        let config: ToolConfig = serde_json::from_str(&config_str)?;

        let tools_len = config.tools.len();
        for alias_config in config.tools {
            // Create parameter mapper from config
            let mapper: Option<Box<dyn ParameterMapper>> =
                if let Some(mapping) = alias_config.parameter_mapping {
                    Some(Box::new(ConfigParameterMapper { mapping }))
                } else {
                    None
                };

            self.register_alias(&alias_config.name, &alias_config.alias_for, mapper)
                .await;
        }

        info!("Loaded {} tool aliases from config", tools_len);
        Ok(())
    }

    /// Get list of all available tools (including aliases)
    pub async fn list_tools(&self) -> Vec<String> {
        let mut tool_names = Vec::new();

        let tools = self.tools.read().await;
        tool_names.extend(tools.keys().cloned());

        let aliases = self.aliases.read().await;
        tool_names.extend(aliases.keys().cloned());

        tool_names.sort();
        tool_names
    }
}

/// Parameter mapper that uses a simple key mapping from config
pub struct ConfigParameterMapper {
    mapping: HashMap<String, String>,
}

impl ParameterMapper for ConfigParameterMapper {
    fn map(&self, args: &serde_json::Value) -> serde_json::Value {
        let mut result = serde_json::Map::new();

        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                let target_key = self.mapping.get(key).unwrap_or(key);
                result.insert(target_key.clone(), value.clone());
            }
        }

        serde_json::Value::Object(result)
    }
}

/// Parameter mapper specifically for container.exec -> bash
pub struct ContainerExecMapper;

impl ParameterMapper for ContainerExecMapper {
    fn map(&self, args: &serde_json::Value) -> serde_json::Value {
        // Extract command from cmd array or command field
        let command = if let Some(cmd_array) = args.get("cmd").and_then(|v| v.as_array()) {
            // Join array elements into a single command
            cmd_array
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<&str>>()
                .join(" ")
        } else if let Some(cmd_str) = args.get("command").and_then(|v| v.as_str()) {
            cmd_str.to_string()
        } else {
            warn!("container.exec call with no cmd or command field");
            String::new()
        };

        serde_json::json!({
            "command": command
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTool {
        name: String,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A test tool"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "test_param": {"type": "string"}
                },
                "required": ["test_param"]
            })
        }

        async fn execute(&self, _args: &serde_json::Value) -> Result<String> {
            Ok("test result".to_string())
        }
    }

    #[tokio::test]
    async fn test_tool_registration() {
        let registry = ToolRegistry::new();
        let tool = Box::new(TestTool {
            name: "test_tool".to_string(),
        });

        registry.register_tool(tool).await;

        let found = registry.get_tool("test_tool").await;
        assert_eq!(found, Some("test_tool".to_string()));
    }

    #[tokio::test]
    async fn test_tool_alias() {
        let registry = ToolRegistry::new();
        let tool = Box::new(TestTool {
            name: "original".to_string(),
        });

        registry.register_tool(tool).await;
        registry.register_alias("alias", "original", None).await;

        let found = registry.get_tool("alias").await;
        assert_eq!(found, Some("original".to_string()));
    }

    #[tokio::test]
    async fn test_container_exec_mapper() {
        let mapper = ContainerExecMapper;

        let args = serde_json::json!({
            "cmd": ["bash", "-lc", "echo hello"]
        });

        let mapped = mapper.map(&args);
        assert_eq!(mapped["command"].as_str(), Some("bash -lc echo hello"));
    }
}
