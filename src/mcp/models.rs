use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerInput {
    pub name: String,
    pub base_url: String,
    pub auth_type: Option<String>,
    pub auth_payload: Option<Value>,
    #[serde(default)]
    pub sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerResponse {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub auth_type: Option<String>,
    pub auth_payload: Option<Value>,
    pub status: String,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResponse {
    pub id: Uuid,
    pub server_id: Uuid,
    pub server_name: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub metadata: Option<Value>,
    pub version: Option<String>,
    pub created_at: String,
    pub examples: Option<Vec<ToolExampleResponse>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvokeRequest {
    pub server: Option<String>,
    pub server_id: Option<Uuid>,
    pub tool: String,
    pub arguments: Option<Value>,
    pub sandbox_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvocationResponse {
    pub id: Uuid,
    pub status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolExampleInput {
    pub title: Option<String>,
    pub body: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolExampleResponse {
    pub id: Uuid,
    pub tool_id: Uuid,
    pub title: Option<String>,
    pub body: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub metadata: Option<Value>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpToolSyncResponse {
    pub tools: Vec<McpToolDescriptor>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AuthPayload {
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default)]
    pub basic_username: Option<String>,
    #[serde(default)]
    pub basic_password: Option<String>,
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchInvokeCall {
    pub tool: String,
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchInvokeRequest {
    pub server: Option<String>,
    pub server_id: Option<Uuid>,
    pub calls: Vec<BatchInvokeCall>,
    pub sandbox_id: Option<Uuid>,
    #[serde(default)]
    pub write_trace: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchInvokeResult {
    pub invocation_id: Uuid,
    pub tool: String,
    pub status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchInvokeResponse {
    pub batch_id: Uuid,
    pub server_id: Uuid,
    pub results: Vec<BatchInvokeResult>,
}
