use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct McpTarget {
    pub name: &'static str,
    pub config_path: PathBuf,
}

pub fn get_claude_desktop_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join("Library/Application Support/Claude/claude_desktop_config.json"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(appdata).join("Claude/claude_desktop_config.json"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config/Claude/claude_desktop_config.json"))
    }
}

pub fn get_cursor_global_config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home).join(".cursor/mcp.json"))
}

pub fn get_all_targets() -> Vec<McpTarget> {
    let mut targets = Vec::new();
    if let Some(claude) = get_claude_desktop_config_path() {
        targets.push(McpTarget {
            name: "Claude Desktop",
            config_path: claude,
        });
    }
    if let Some(cursor) = get_cursor_global_config_path() {
        targets.push(McpTarget {
            name: "Cursor",
            config_path: cursor,
        });
    }
    targets
}

fn resolve_binary_path() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = fs::canonicalize(&exe) {
            return canonical.to_string_lossy().to_string();
        }
        return exe.to_string_lossy().to_string();
    }
    "frontlane-serp".to_string()
}

pub fn install_mcp(target_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = resolve_binary_path();
    let filter = target_filter.to_lowercase();
    let targets = get_all_targets();

    let mut configured_count = 0;

    println!("\n  📦 Frontlane SERP — 1-Click MCP Setup");
    println!("  ──────────────────────────────────────────");
    println!("  Executable: {}\n", bin_path);

    for target in targets {
        if filter != "all" && !target.name.to_lowercase().contains(&filter) {
            continue;
        }

        println!("  Target: {}", target.name);
        println!("  Config: {}", target.config_path.display());

        let mut config_val: Value = if target.config_path.exists() {
            let content = fs::read_to_string(&target.config_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
        } else {
            json!({})
        };

        if !config_val.is_object() {
            config_val = json!({});
        }

        let mcp_servers = config_val
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert_with(|| json!({}));

        if !mcp_servers.is_object() {
            *mcp_servers = json!({});
        }

        let entry = json!({
            "command": bin_path,
            "args": ["mcp"]
        });

        mcp_servers
            .as_object_mut()
            .unwrap()
            .insert("frontlane-serp".to_string(), entry);

        if let Some(parent) = target.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let formatted = serde_json::to_string_pretty(&config_val)?;
        fs::write(&target.config_path, formatted)?;

        println!("  Status: ✔ Successfully configured 'frontlane-serp' entry\n");
        configured_count += 1;
    }

    if configured_count == 0 {
        println!(
            "  ⚠ No matching MCP client config found for filter '{}'.",
            target_filter
        );
    } else {
        println!("  🎉 Done! Please restart your AI client (Claude Desktop / Cursor) to activate the tools.");
        println!("  Exposed tools: serp_search, mega_search, crawl_site, extract_content, check_rank, suggest_keywords.\n");
    }

    Ok(())
}

pub fn uninstall_mcp(target_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let filter = target_filter.to_lowercase();
    let targets = get_all_targets();
    let mut removed_count = 0;

    println!("\n  🗑  Frontlane SERP — MCP Uninstall");
    println!("  ──────────────────────────────────────────\n");

    for target in targets {
        if filter != "all" && !target.name.to_lowercase().contains(&filter) {
            continue;
        }

        if !target.config_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&target.config_path)?;
        if let Ok(mut config_val) = serde_json::from_str::<Value>(&content) {
            if let Some(servers) = config_val
                .get_mut("mcpServers")
                .and_then(|s| s.as_object_mut())
            {
                if servers.remove("frontlane-serp").is_some() {
                    let formatted = serde_json::to_string_pretty(&config_val)?;
                    fs::write(&target.config_path, formatted)?;
                    println!("  ✔ Removed 'frontlane-serp' from {}", target.name);
                    removed_count += 1;
                }
            }
        }
    }

    if removed_count == 0 {
        println!("  No 'frontlane-serp' entry found in configured MCP clients.");
    } else {
        println!("\n  Uninstallation complete. Restart your AI client to apply changes.\n");
    }

    Ok(())
}

pub fn status_mcp() -> Result<(), Box<dyn std::error::Error>> {
    let targets = get_all_targets();

    println!("\n  🔍 Frontlane SERP — MCP Client Status");
    println!("  ──────────────────────────────────────────");

    for target in targets {
        print!("  • {:<16} : ", target.name);
        if !target.config_path.exists() {
            println!("Not configured (file does not exist)");
            println!("    Path: {}", target.config_path.display());
            continue;
        }

        let content = fs::read_to_string(&target.config_path).unwrap_or_default();
        let is_installed = if let Ok(val) = serde_json::from_str::<Value>(&content) {
            val.get("mcpServers")
                .and_then(|s| s.get("frontlane-serp"))
                .is_some()
        } else {
            false
        };

        if is_installed {
            println!("✔ Installed");
        } else {
            println!("○ Config exists, but 'frontlane-serp' is not added");
        }
        println!("    Path: {}", target.config_path.display());
    }

    println!("\n  Tip: Run `frontlane-serp mcp install` to automatically configure all clients.\n");
    Ok(())
}
