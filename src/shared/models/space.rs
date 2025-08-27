use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    pub name: String,
    pub description: Option<String>,
    pub settings: serde_json::Value,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSecretWithValue {
    pub space: String,
    pub key_name: String,
    pub encrypted_value: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}