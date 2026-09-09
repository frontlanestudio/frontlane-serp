use std::net::TcpListener;
use std::sync::Arc;
use std::time::Instant;

use crate::config::AppConfig;
use crate::core::engine::SearchEngine;
use crate::core::http_client::HttpClient;
use crate::core::types::Query;
use crate::mcp::installer::get_all_targets;

pub struct DoctorOptions {
    pub skip_engines: bool,
    pub specific_engine: Option<String>,
}

pub async fn run_doctor(
    config: &AppConfig,
    engines: &[Arc<dyn SearchEngine>],
    _http_client: &HttpClient,
    opts: DoctorOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n  🩺 Frontlane SERP — Environment & Engine Doctor");
    println!("  ─────────────────────────────────────────────────────────────");

    // 1. System Info
    println!("  System:");
    println!("    • Version:         0.1.0 (API v2.1)");
    println!(
        "    • Platform:        {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    if let Ok(exe) = std::env::current_exe() {
        println!("    • Executable:      {}", exe.display());
    }

    // 2. Configuration & Network
    println!("\n  Security & Networking:");
    #[cfg(feature = "impersonate")]
    {
        if config.app.browser_impersonation {
            println!(
                "    • TLS Engine:      ✔ Browser TLS/JA3 impersonation enabled (wreq/BoringSSL)"
            );
        } else {
            println!("    • TLS Engine:      ○ Standard TLS (impersonation disabled in config)");
        }
    }
    #[cfg(not(feature = "impersonate"))]
    {
        println!("    • TLS Engine:      ○ Standard TLS (reqwest rustls-tls)");
    }

    if let Some(ref p) = config.proxies.global {
        println!("    • Global Proxy:    ✔ Configured ({})", p);
    } else {
        println!("    • Proxy Mode:      Direct connection (no global proxy set)");
    }

    // Port check
    let port = config.server.port;
    let host = &config.server.host;
    let bind_addr = format!("{}:{}", host, port);
    match TcpListener::bind(&bind_addr) {
        Ok(_) => {
            println!(
                "    • Local Port:      ✔ Port {} is available to bind",
                port
            );
        }
        Err(_) => {
            // Check if it's our own server running
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(800))
                .build()
                .unwrap_or_default();
            let check_url = format!(
                "http://{}:{}/health",
                if host == "0.0.0.0" { "127.0.0.1" } else { host },
                port
            );
            if let Ok(resp) = client.get(&check_url).send().await {
                if resp.status().is_success() {
                    println!("    • Local Port:      ✔ Port {} is active (Frontlane SERP server running at http://{}:{})", port, host, port);
                } else {
                    println!(
                        "    • Local Port:      ⚠ Port {} is in use by another process",
                        port
                    );
                    if cfg!(target_os = "macos") && port == 7000 {
                        println!("                       (Tip: macOS AirPlay Receiver uses port 7000; run with `--port 7070` or disable AirPlay Receiver in System Settings)");
                    }
                }
            } else {
                println!(
                    "    • Local Port:      ⚠ Port {} is in use by another process",
                    port
                );
                if cfg!(target_os = "macos") && port == 7000 {
                    println!("                       (Tip: macOS AirPlay Receiver uses port 7000; run with `--port 7070` or disable AirPlay Receiver in System Settings)");
                }
            }
        }
    }

    // 3. Search Engine Connectivity
    println!("\n  Search Engine Connectivity:");
    if opts.skip_engines {
        println!("    • Engine Probing:  Skipped (--skip-engines)");
    } else {
        let test_query = Query {
            text: "rust async".to_string(),
            lang_code: "en".to_string(),
            region: "us".to_string(),
            date_interval: String::new(),
            filetype: String::new(),
            site: String::new(),
            limit: 5,
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

        let target_engine_names: Vec<&str> = if let Some(ref e) = opts.specific_engine {
            vec![e.as_str()]
        } else {
            vec!["duckduckgo", "bing", "google", "crates", "hackernews"]
        };

        let mut all_ok = true;

        for eng_name in target_engine_names {
            let eng = engines
                .iter()
                .find(|e| e.name().eq_ignore_ascii_case(eng_name));
            if let Some(engine) = eng {
                let start = Instant::now();
                match engine.search(&test_query).await {
                    Ok(res) => {
                        let duration = start.elapsed().as_millis();
                        println!(
                            "    • {:<14} : ✔ OK ({}ms, {} results)",
                            engine.name(),
                            duration,
                            res.len()
                        );
                    }
                    Err(e) => {
                        all_ok = false;
                        print_engine_probe_error(engine.name(), &e.to_string());
                    }
                }
            } else {
                println!(
                    "    • {:<14} : ❓ Engine not found in active build",
                    eng_name
                );
            }
        }

        if !all_ok {
            println!("\n    💡 Tip: If any search engine is blocked or blocked by CAPTCHA:");
            println!("       1. Deploy free rotating edge proxies: `frontlane-serp flareprox create --count 3`");
            println!("       2. Or configure captcha solving in config.yaml (`captcha.apikey`)");
        }
    }

    // 4. MCP Server Integration Status
    println!("\n  AI Agent Integration (MCP):");
    let targets = get_all_targets();
    for target in targets {
        if !target.config_path.exists() {
            println!(
                "    • {:<14} : ○ Not configured ({})",
                target.name,
                target.config_path.display()
            );
            continue;
        }
        let content = std::fs::read_to_string(&target.config_path).unwrap_or_default();
        let configured = content.contains("\"frontlane-serp\"");
        if configured {
            println!(
                "    • {:<14} : ✔ Ready & configured in {}",
                target.name,
                target.config_path.display()
            );
        } else {
            println!(
                "    • {:<14} : ○ Not configured (Run `frontlane-serp mcp install` to setup)",
                target.name
            );
        }
    }

    println!("\n  ─────────────────────────────────────────────────────────────");
    println!(
        "  Documentation: http://{}:{}/docs (Interactive Swagger UI)",
        host, port
    );
    println!("  Quick Search:  frontlane-serp search duckduckgo \"rust async\"\n");

    Ok(())
}

fn print_engine_probe_error(engine_name: &str, err_str: &str) {
    let lower = err_str.to_lowercase();
    if lower.contains("captcha") {
        println!("    • {:<14} : ⚠ CAPTCHA / Bot Gate detected", engine_name);
    } else if lower.contains("rate") {
        println!("    • {:<14} : ⚠ Rate limited", engine_name);
    } else {
        println!("    • {:<14} : ❌ Error: {}", engine_name, err_str);
    }
}
