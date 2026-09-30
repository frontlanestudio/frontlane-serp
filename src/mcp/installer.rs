use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

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

/// Merge the `frontlane-serp` server entry into an MCP client config.
///
/// `existing` is the current file contents (`None` if the file does not exist).
/// Returns the new pretty-printed JSON, or a human-readable reason why the
/// existing config can't be safely modified. Never discards user data: invalid
/// JSON, a non-object root, or a non-object `mcpServers` value are all errors.
fn merge_server_entry(existing: Option<&str>, bin_path: &str) -> Result<String, String> {
    let mut config_val: Value = match existing {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text)
            .map_err(|e| format!("existing config is not valid JSON ({e})"))?,
        _ => json!({}),
    };

    let root = config_val
        .as_object_mut()
        .ok_or_else(|| "existing config root is not a JSON object".to_string())?;

    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "existing \"mcpServers\" value is not a JSON object".to_string())?;

    servers.insert(
        "frontlane-serp".to_string(),
        json!({
            "command": bin_path,
            "args": ["mcp"]
        }),
    );

    serde_json::to_string_pretty(&config_val).map_err(|e| e.to_string())
}

/// Path of the backup written next to a config before it is modified.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| "config.json".into());
    name.push(".bak");
    path.with_file_name(name)
}

/// Write `contents` to `path` atomically (temp file in the same directory,
/// then rename), so a crash mid-write can't leave a truncated config.
/// Symlinked configs (e.g. managed dotfiles) are written through to their target.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.json".to_string());
    let tmp = dir.join(format!(".{file_name}.frontlane-tmp-{}", std::process::id()));

    fs::write(&tmp, contents)?;
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

pub fn install_mcp(target_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = resolve_binary_path();
    let filter = target_filter.to_lowercase();
    let targets = get_all_targets();

    let mut configured_count = 0;
    let mut skipped: Vec<&'static str> = Vec::new();

    println!("\n  📦 Frontlane SERP — 1-Click MCP Setup");
    println!("  ──────────────────────────────────────────");
    println!("  Executable: {}\n", bin_path);

    for target in targets {
        if filter != "all" && !target.name.to_lowercase().contains(&filter) {
            continue;
        }

        println!("  Target: {}", target.name);
        println!("  Config: {}", target.config_path.display());

        let existing = if target.config_path.exists() {
            Some(fs::read_to_string(&target.config_path)?)
        } else {
            None
        };

        let merged = match merge_server_entry(existing.as_deref(), &bin_path) {
            Ok(merged) => merged,
            Err(reason) => {
                println!("  Status: ✘ Skipped — {reason}.");
                println!("          The file was left untouched. Fix it by hand, then re-run");
                println!("          `frontlane-serp mcp install`.\n");
                skipped.push(target.name);
                continue;
            }
        };

        if existing.as_deref() == Some(merged.as_str()) {
            println!("  Status: ✔ Already configured\n");
            configured_count += 1;
            continue;
        }

        if existing.is_some() {
            let backup = backup_path(&target.config_path);
            fs::copy(&target.config_path, &backup)?;
            println!("  Backup: {}", backup.display());
        }

        write_atomic(&target.config_path, &merged)?;

        println!("  Status: ✔ Successfully configured 'frontlane-serp' entry\n");
        configured_count += 1;
    }

    if configured_count == 0 && skipped.is_empty() {
        println!(
            "  ⚠ No matching MCP client config found for filter '{}'.",
            target_filter
        );
    } else if configured_count > 0 {
        println!("  🎉 Done! Please restart your AI client (Claude Desktop / Cursor) to activate the tools.");
        println!("  Exposed tools: serp_search, mega_search, crawl_site, extract_content, check_rank, suggest_keywords.\n");
    }

    if !skipped.is_empty() {
        return Err(format!(
            "could not update MCP config for: {} (see messages above)",
            skipped.join(", ")
        )
        .into());
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
        let mut config_val = match serde_json::from_str::<Value>(&content) {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "  ⚠ Skipped {}: config is not valid JSON ({e}); left untouched",
                    target.name
                );
                continue;
            }
        };
        if let Some(servers) = config_val
            .get_mut("mcpServers")
            .and_then(|s| s.as_object_mut())
        {
            if servers.remove("frontlane-serp").is_some() {
                let formatted = serde_json::to_string_pretty(&config_val)?;
                fs::copy(&target.config_path, backup_path(&target.config_path))?;
                write_atomic(&target.config_path, &formatted)?;
                println!("  ✔ Removed 'frontlane-serp' from {}", target.name);
                removed_count += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    const BIN: &str = "/usr/local/bin/frontlane-serp";

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("merge output is valid JSON")
    }

    #[test]
    fn creates_config_when_missing_or_empty() {
        for existing in [None, Some(""), Some("  \n")] {
            let out = parse(&merge_server_entry(existing, BIN).unwrap());
            assert_eq!(out["mcpServers"]["frontlane-serp"]["command"], BIN);
            assert_eq!(out["mcpServers"]["frontlane-serp"]["args"], json!(["mcp"]));
        }
    }

    #[test]
    fn preserves_other_servers_and_settings() {
        let existing = r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#;
        let out = parse(&merge_server_entry(Some(existing), BIN).unwrap());
        assert_eq!(out["theme"], "dark");
        assert_eq!(out["mcpServers"]["other"]["command"], "x");
        assert_eq!(out["mcpServers"]["frontlane-serp"]["command"], BIN);
    }

    #[test]
    fn replaces_stale_frontlane_entry() {
        let existing = r#"{"mcpServers":{"frontlane-serp":{"command":"/old/path"}}}"#;
        let out = parse(&merge_server_entry(Some(existing), BIN).unwrap());
        assert_eq!(out["mcpServers"]["frontlane-serp"]["command"], BIN);
    }

    #[test]
    fn refuses_invalid_json() {
        // Trailing comma, as hand-edited configs often have.
        let existing = r#"{"mcpServers":{"other":{"command":"x"},}}"#;
        let err = merge_server_entry(Some(existing), BIN).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn refuses_non_object_root() {
        assert!(merge_server_entry(Some("[1,2,3]"), BIN).is_err());
    }

    #[test]
    fn refuses_non_object_mcp_servers() {
        let existing = r#"{"mcpServers":["other"]}"#;
        assert!(merge_server_entry(Some(existing), BIN).is_err());
    }

    #[test]
    fn backup_path_appends_suffix() {
        let p = Path::new("/a/b/claude_desktop_config.json");
        assert_eq!(
            backup_path(p),
            Path::new("/a/b/claude_desktop_config.json.bak")
        );
    }

    #[test]
    fn write_atomic_replaces_contents_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, "old").unwrap();

        write_atomic(&path, "new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("frontlane-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn write_atomic_creates_missing_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/dir/mcp.json");
        write_atomic(&path, "{}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
    }
}
