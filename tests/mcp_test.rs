use frontlane_serp::mcp::protocol::*;
use frontlane_serp::mcp::server::get_available_tools;
use serde_json::json;

#[test]
fn test_mcp_tools_list() {
    let tools = get_available_tools();
    assert_eq!(tools.len(), 5);

    let names: Vec<String> = tools.into_iter().map(|t| t.name).collect();
    assert!(names.contains(&"serp_search".to_string()));
    assert!(names.contains(&"check_rank".to_string()));
    assert!(names.contains(&"suggest_keywords".to_string()));
    assert!(names.contains(&"extract_content".to_string()));
    assert!(names.contains(&"mega_search".to_string()));
}

#[test]
fn test_mcp_json_rpc_serialization() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let serialized = serde_json::to_string(&req).unwrap();
    assert!(serialized.contains(r#""jsonrpc":"2.0""#));
    assert!(serialized.contains(r#""method":"tools/list""#));

    let resp = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        result: Some(json!({"status": "ok"})),
        error: None,
    };

    let resp_str = serde_json::to_string(&resp).unwrap();
    assert!(resp_str.contains(r#""result":{"status":"ok"}"#));
}
