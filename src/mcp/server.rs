use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::core::engine::SearchEngine;
use crate::core::response_builder::enrich_result;
use crate::core::types::{Query, ResultItem};
use crate::extract::Extractor;
use crate::mcp::protocol::*;
use crate::mega::MegaSearcher;
use crate::rank::{probe_engine_rank, DeviceType, DomainMatchMode, RankRequest, RankStrategy};
use crate::suggest::SuggestClient;

pub fn get_available_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "serp_search".to_string(),
            description: "Search Google, Bing, DuckDuckGo, Baidu, Yandex, Ecosia, HackerNews, GitHub, Crates.io, or Wikipedia and return structured results."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "engine": {
                        "type": "string",
                        "description": "Search engine: google, bing, duckduckgo, yandex, baidu, ecosia, hackernews, github, crates, wikipedia",
                        "default": "google"
                    },
                    "query": { "type": "string", "description": "Search query keywords" },
                    "limit": { "type": "integer", "description": "Number of results (1-50)", "default": 10 },
                    "region": { "type": "string", "description": "ISO-3166 region code (e.g. US, UK, DE)", "default": "US" },
                    "lang": { "type": "string", "description": "Language code (e.g. en, de, fr)", "default": "en" }
                },
                "required": ["query"]
            }),
        },
        McpTool {
            name: "check_rank".to_string(),
            description: "Intelligently check search engine rank for a target domain or URL, tracking SERP features and citations."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Target domain or URL (e.g. example.com, blog.example.com)" },
                    "query": { "type": "string", "description": "Keyword to check" },
                    "engine": { "type": "string", "description": "Search engine: google, bing, duckduckgo", "default": "google" },
                    "strategy": { "type": "string", "enum": ["smart", "basic", "custom"], "default": "smart" },
                    "last_rank": { "type": "integer", "description": "Last known position (for smart neighbor probing)", "default": 0 },
                    "device": { "type": "string", "enum": ["desktop", "mobile"], "default": "desktop" },
                    "match_mode": { "type": "string", "enum": ["subdomain", "exact", "wildcard"], "default": "subdomain" }
                },
                "required": ["target", "query"]
            }),
        },
        McpTool {
            name: "suggest_keywords".to_string(),
            description: "Fetch instant Google/Bing/DDG autocomplete suggestions for keyword ideas and search intent."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Seed keyword" },
                    "engine": { "type": "string", "enum": ["google", "bing", "duckduckgo", "ecosia"], "default": "google" },
                    "region": { "type": "string", "default": "us" },
                    "lang": { "type": "string", "default": "en" }
                },
                "required": ["query"]
            }),
        },
        McpTool {
            name: "extract_content".to_string(),
            description: "Extract clean, reader-friendly markdown content, JSON-LD schemas, and metadata from any web page URL or llms.txt."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Target web page URL" },
                    "llms_txt": { "type": "boolean", "description": "Check /llms.txt first", "default": true }
                },
                "required": ["url"]
            }),
        },
        McpTool {
            name: "mega_search".to_string(),
            description: "Perform aggregated multi-engine search across multiple engines with Reciprocal Rank Fusion (RRF) scoring, consensus metrics, and clustering."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "engines": { "type": "array", "items": { "type": "string" }, "default": ["google", "bing", "duckduckgo"] },
                    "limit": { "type": "integer", "default": 10 },
                    "mode": { "type": "string", "enum": ["balanced", "any", "fast"], "default": "balanced" }
                },
                "required": ["query"]
            }),
        },
        McpTool {
            name: "crawl_site".to_string(),
            description: "Asynchronously crawl a website starting from a URL with depth/page limits, domain boundary filtering, and robots.txt politeness."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Starting URL to crawl" },
                    "max_depth": { "type": "integer", "description": "Maximum crawl depth (1-5)", "default": 2 },
                    "max_pages": { "type": "integer", "description": "Maximum pages to crawl (1-50)", "default": 10 },
                    "allowed_domains": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional domain filter (defaults to starting URL host)"
                    },
                    "respect_robots": { "type": "boolean", "description": "Respect robots.txt rules", "default": true }
                },
                "required": ["url"]
            }),
        },
        McpTool {
            name: "serp_extract_contacts".to_string(),
            description: "Extract all phone numbers, postal addresses, email addresses, and social profile links from a web page or entire site/domain."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Target web page URL or domain" },
                    "crawl": { "type": "boolean", "description": "Whether to crawl internal contact/about/location pages across the domain", "default": false },
                    "max_pages": { "type": "integer", "description": "Maximum pages to scan (1-50)", "default": 10 }
                },
                "required": ["url"]
            }),
        },
    ]
}

pub async fn handle_tool_call(
    name: &str,
    args: serde_json::Value,
    engines: &HashMap<String, Arc<dyn SearchEngine>>,
    mega: &Arc<MegaSearcher>,
    extractor: &Arc<Extractor>,
    suggest: &SuggestClient,
    crawler: &Arc<crate::crawl::Crawler>,
) -> McpToolCallResult {
    match name {
        "serp_search" => {
            let engine_name = args
                .get("engine")
                .and_then(|v| v.as_str())
                .unwrap_or("google")
                .to_lowercase();
            let query_text = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let region = args
                .get("region")
                .and_then(|v| v.as_str())
                .unwrap_or("US")
                .to_string();
            let lang = args
                .get("lang")
                .and_then(|v| v.as_str())
                .unwrap_or("en")
                .to_string();

            let norm_engine = match engine_name.as_str() {
                "ddg" | "duck" => "duckduckgo",
                other => other,
            };

            let engine = match engines.get(norm_engine) {
                Some(e) => e,
                None => {
                    return McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: format!("Unknown engine: {}", engine_name),
                        }],
                        is_error: true,
                    };
                }
            };

            let q = Query {
                text: query_text.to_string(),
                lang_code: lang,
                region,
                date_interval: String::new(),
                filetype: String::new(),
                site: String::new(),
                limit,
                start: 0,
                filter: true,
                features: true,
                extract: false,
                extract_top: 0,
                extract_mode: "auto".to_string(),
                extract_min_runes: 0,
                proxy_url: None,
                proxy_country: None,
                proxy_class: None,
                proxy_provider: None,
                proxy_session_id: None,
                proxy_override: None,
                insecure: true,
                guard_private_networks: false,
            };

            match engine.search(&q).await {
                Ok(raw) => {
                    let mut items: Vec<ResultItem> = Vec::new();
                    for r in raw {
                        items.push(enrich_result(r, norm_engine, 0));
                    }
                    let json_out = serde_json::to_string_pretty(&items).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Search error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "check_rank" => {
            let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
            let query_text = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let engine_name = args
                .get("engine")
                .and_then(|v| v.as_str())
                .unwrap_or("google")
                .to_lowercase();
            let strategy_str = args
                .get("strategy")
                .and_then(|v| v.as_str())
                .unwrap_or("smart");
            let last_rank = args.get("last_rank").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let device_str = args
                .get("device")
                .and_then(|v| v.as_str())
                .unwrap_or("desktop");
            let match_str = args
                .get("match_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("subdomain");

            let strategy = match strategy_str {
                "basic" => RankStrategy::Basic,
                "custom" => RankStrategy::Custom,
                _ => RankStrategy::Smart,
            };

            let device = match device_str {
                "mobile" => DeviceType::Mobile,
                _ => DeviceType::Desktop,
            };

            let match_mode = match match_str {
                "exact" => DomainMatchMode::Exact,
                "wildcard" => DomainMatchMode::Wildcard,
                _ => DomainMatchMode::Subdomain,
            };

            let engine = match engines.get(&engine_name) {
                Some(e) => e.clone(),
                None => {
                    return McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: format!("Unknown engine: {}", engine_name),
                        }],
                        is_error: true,
                    };
                }
            };

            let req = RankRequest {
                target: target.to_string(),
                q: query_text.to_string(),
                strategy,
                last_rank,
                pagination_limit: 5,
                smart_full_fallback: true,
                r#match: match_mode,
                device,
                region: "US".to_string(),
                lang: "en".to_string(),
            };

            match probe_engine_rank(engine, &req).await {
                Ok(resp) => {
                    let json_out = serde_json::to_string_pretty(&resp).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Rank probe error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "suggest_keywords" => {
            let query_text = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let engine_name = args
                .get("engine")
                .and_then(|v| v.as_str())
                .unwrap_or("google");
            let region = args.get("region").and_then(|v| v.as_str()).unwrap_or("us");
            let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en");

            match suggest.suggest(engine_name, query_text, lang, region).await {
                Ok(resp) => {
                    let json_out = serde_json::to_string_pretty(&resp).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Suggest error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "extract_content" => {
            let target_url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let llms_txt = args
                .get("llms_txt")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            match extractor.extract(target_url, llms_txt).await {
                Ok(content) => {
                    let json_out = serde_json::to_string_pretty(&content).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Extraction error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "mega_search" => {
            let query_text = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let mode = args
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("balanced");

            let engines_list: Vec<String> =
                if let Some(arr) = args.get("engines").and_then(|v| v.as_array()) {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    vec![
                        "google".to_string(),
                        "bing".to_string(),
                        "duckduckgo".to_string(),
                    ]
                };

            let q = Query {
                text: query_text.to_string(),
                lang_code: "en".to_string(),
                region: "US".to_string(),
                date_interval: String::new(),
                filetype: String::new(),
                site: String::new(),
                limit,
                start: 0,
                filter: true,
                features: true,
                extract: false,
                extract_top: 0,
                extract_mode: "auto".to_string(),
                extract_min_runes: 0,
                proxy_url: None,
                proxy_country: None,
                proxy_class: None,
                proxy_provider: None,
                proxy_session_id: None,
                proxy_override: None,
                insecure: true,
                guard_private_networks: false,
            };

            match mega.search(&q, &engines_list, mode).await {
                Ok(env) => {
                    let json_out = serde_json::to_string_pretty(&env.results).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Mega search error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "crawl_site" => {
            let start_url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
            let max_pages = args.get("max_pages").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let respect_robots = args
                .get("respect_robots")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let allowed_domains =
                if let Some(arr) = args.get("allowed_domains").and_then(|v| v.as_array()) {
                    arr.iter()
                        .filter_map(|d| d.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    vec![]
                };

            let options = crate::crawl::CrawlOptions {
                start_url: start_url.to_string(),
                max_depth,
                max_pages,
                concurrency: 4,
                allowed_domains,
                respect_robots,
                extract_content: true,
            };

            match crawler.crawl(options).await {
                Ok(res) => {
                    let json_out = serde_json::to_string_pretty(&res).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Crawl error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        "serp_extract_contacts" => {
            let target_url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let crawl = args.get("crawl").and_then(|v| v.as_bool()).unwrap_or(false);
            let max_pages = args.get("max_pages").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

            match crate::extract::scan_contacts(
                crawler.http_client(),
                target_url,
                crawl,
                max_pages,
                2,
            )
            .await
            {
                Ok(contacts) => {
                    let json_out = serde_json::to_string_pretty(&contacts).unwrap_or_default();
                    McpToolCallResult {
                        content: vec![McpToolCallContent {
                            r#type: "text".to_string(),
                            text: json_out,
                        }],
                        is_error: false,
                    }
                }
                Err(e) => McpToolCallResult {
                    content: vec![McpToolCallContent {
                        r#type: "text".to_string(),
                        text: format!("Contacts extraction error: {}", e),
                    }],
                    is_error: true,
                },
            }
        }
        _ => McpToolCallResult {
            content: vec![McpToolCallContent {
                r#type: "text".to_string(),
                text: format!("Unknown tool: {}", name),
            }],
            is_error: true,
        },
    }
}

pub async fn run_stdio_mcp_server(
    engines: HashMap<String, Arc<dyn SearchEngine>>,
    mega: Arc<MegaSearcher>,
    extractor: Arc<Extractor>,
    suggest: SuggestClient,
    crawler: Arc<crate::crawl::Crawler>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };

        match req.method.as_str() {
            "initialize" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "frontlane-serp",
                            "version": "0.1.0"
                        }
                    })),
                    error: None,
                };
                let mut out = serde_json::to_string(&resp)?;
                out.push('\n');
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
            }
            "notifications/initialized" => {
                // No reply required
            }
            "tools/list" => {
                let tools = get_available_tools();
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(json!({ "tools": tools })),
                    error: None,
                };
                let mut out = serde_json::to_string(&resp)?;
                out.push('\n');
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
            }
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = params.get("arguments").cloned().unwrap_or(json!({}));

                let call_res = handle_tool_call(
                    tool_name, tool_args, &engines, &mega, &extractor, &suggest, &crawler,
                )
                .await;

                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(serde_json::to_value(&call_res).unwrap_or(json!({}))),
                    error: None,
                };
                let mut out = serde_json::to_string(&resp)?;
                out.push('\n');
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
            }
            _ => {
                if req.id.is_some() {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {}", req.method),
                            data: None,
                        }),
                    };
                    let mut out = serde_json::to_string(&resp)?;
                    out.push('\n');
                    stdout.write_all(out.as_bytes()).await?;
                    stdout.flush().await?;
                }
            }
        }
    }

    Ok(())
}
