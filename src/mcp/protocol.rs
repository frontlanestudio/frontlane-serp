use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallContent {
    pub r#type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpToolCallContent>,
    #[serde(default)]
    pub is_error: bool,
}

impl McpToolCallResult {
    pub fn success_json<T: Serialize>(value: &T) -> Self {
        let text = serde_json::to_string_pretty(value).unwrap_or_default();
        Self {
            content: vec![McpToolCallContent {
                r#type: "text".to_string(),
                text,
            }],
            is_error: false,
        }
    }

    pub fn error(message: impl std::fmt::Display) -> Self {
        Self {
            content: vec![McpToolCallContent {
                r#type: "text".to_string(),
                text: message.to_string(),
            }],
            is_error: true,
        }
    }
}
