use std::time::Duration;
use reqwest::Client;

use crate::config::AppConfig;
use crate::flareprox::client::{resolve_placement, CloudflareClient, FlareProxDeployment};
use crate::flareprox::FlareProxError;

#[derive(Debug, Clone)]
pub struct EdgeDeployOptions {
    pub name: Option<String>,
    pub proxies: usize,
    pub region: Option<String>,
    pub auto_recycle: bool,
}

pub async fn run_edge_deploy(
    config: &AppConfig,
    token_opt: Option<String>,
    account_opt: Option<String>,
    opts: EdgeDeployOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cfg_token, cfg_account) = config.flareprox.resolved_credentials();
    let token = token_opt.or(cfg_token).ok_or_else(|| {
        FlareProxError::MissingCredentials("CLOUDFLARE_API_TOKEN is required. Set via env or config.yaml".to_string())
    })?;

    let account = match account_opt.or(cfg_account) {
        Some(a) => a,
        None => CloudflareClient::resolve_account_id(&token).await?,
    };

    let client = CloudflareClient::new(token, account, Some("flareprox".to_string()))?;

    let region_name = opts.region.as_deref().unwrap_or("de");
    let placement = resolve_placement(region_name).unwrap_or_else(|| "aws:eu-central-1".to_string());

    println!("\n  🚀 Frontlane SERP — Cloudflare Edge Stack Deployment");
    println!("  ─────────────────────────────────────────────────────────────");
    println!("  Target Region:       {} (Placement: {})", region_name.to_uppercase(), placement);
    println!("  Proxy Count:         {}", opts.proxies);
    println!("  Auto-Recycling:      {}", if opts.auto_recycle { "Enabled (autonomous on-demand)" } else { "Disabled" });
    println!("  Main Worker Name:    {}\n", opts.name.as_deref().unwrap_or("frontlane-serp-edge"));

    // 1. Deploy Regional Proxy Workers
    let mut proxy_deployments: Vec<FlareProxDeployment> = Vec::new();
    if opts.proxies > 0 {
        println!("  Phase 1: Deploying {} regional FlareProx proxy lane(s)...", opts.proxies);
        for i in 0..opts.proxies {
            print!("    [{}/{}] Deploying proxy in {}... ", i + 1, opts.proxies, region_name.to_uppercase());
            match client.create_worker(None, Some(region_name)).await {
                Ok(dep) => {
                    println!("✔ {}", dep.url);
                    proxy_deployments.push(dep);
                }
                Err(e) => {
                    println!("❌ Failed: {}", e);
                }
            }
        }
    }

    let proxy_urls: Vec<String> = proxy_deployments.iter().map(|d| d.url.clone()).collect();

    // 2. Deploy Main SERP Edge Worker
    println!("\n  Phase 2: Deploying Main SERP Edge Worker...");
    let main_dep = client.deploy_main_edge_worker(opts.name.as_deref(), &proxy_urls, opts.auto_recycle).await?;
    println!("    ✔ Deployed Main Worker: {}", main_dep.url);

    // 3. Health & Verification Probe
    println!("\n  Phase 3: Verifying Edge Deployment...");
    let http = Client::builder().timeout(Duration::from_secs(10)).build()?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let health_url = format!("{}/health", main_dep.url);
    match http.get(&health_url).send().await {
        Ok(res) if res.status().is_success() => {
            println!("    ✔ Health check passed: HTTP 200 OK");
        }
        _ => {
            println!("    ○ Edge route propagating on Cloudflare CDN (may take 5-15s to be globally active)");
        }
    }

    println!("\n  🎉 Deployment Complete!");
    println!("  ─────────────────────────────────────────────────────────────");
    println!("  API Base URL:        {}", main_dep.url);
    println!("  Interactive Docs:    {}/docs", main_dep.url);
    println!("  OpenAPI 3.0 Spec:    {}/openapi.yaml", main_dep.url);
    println!("  Proxies Linked:      {} active in pool ({})", proxy_urls.len(), region_name.to_uppercase());
    println!("\n  Quick Test Commands:");
    println!("    curl \"{}/crates/search?text=tokio&limit=5\"", main_dep.url);
    println!("    curl \"{}/mega/search?text=rust+async&limit=5\"\n", main_dep.url);

    Ok(())
}

pub async fn run_edge_status(
    config: &AppConfig,
    token_opt: Option<String>,
    account_opt: Option<String>,
    name_opt: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cfg_token, cfg_account) = config.flareprox.resolved_credentials();
    let token = token_opt.or(cfg_token).ok_or_else(|| {
        FlareProxError::MissingCredentials("CLOUDFLARE_API_TOKEN is required".to_string())
    })?;

    let account = match account_opt.or(cfg_account) {
        Some(a) => a,
        None => CloudflareClient::resolve_account_id(&token).await?,
    };

    let client = CloudflareClient::new(token, account, Some("flareprox".to_string()))?;
    let main_name = name_opt.unwrap_or_else(|| "frontlane-serp-edge".to_string());
    let subdomain = client.get_subdomain().await?.unwrap_or_default();
    let main_url = format!("https://{}.{}.workers.dev", main_name, subdomain);

    println!("\n  🔍 Cloudflare Edge Stack Status");
    println!("  ─────────────────────────────────────────────────────────────");
    println!("  Main Edge Worker:    {}", main_url);

    let http = Client::builder().timeout(Duration::from_secs(5)).build()?;
    match http.get(format!("{}/health", main_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("  Edge Status:         ✔ Online (HTTP 200)");
        }
        Ok(resp) => {
            println!("  Edge Status:         ⚠ Status HTTP {}", resp.status());
        }
        Err(e) => {
            println!("  Edge Status:         ○ Unreachable / Propagating ({})", e);
        }
    }

    let workers = client.list_workers().await?;
    let proxy_workers: Vec<_> = workers.iter().filter(|w| w.name.starts_with("flareprox")).collect();
    println!("  Active Proxies:      {} FlareProx worker(s) deployed", proxy_workers.len());
    for p in proxy_workers {
        println!("    • {:<32} {}", p.name, p.url);
    }
    println!();

    Ok(())
}

pub async fn run_edge_destroy(
    config: &AppConfig,
    token_opt: Option<String>,
    account_opt: Option<String>,
    name_opt: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cfg_token, cfg_account) = config.flareprox.resolved_credentials();
    let token = token_opt.or(cfg_token).ok_or_else(|| {
        FlareProxError::MissingCredentials("CLOUDFLARE_API_TOKEN is required".to_string())
    })?;

    let account = match account_opt.or(cfg_account) {
        Some(a) => a,
        None => CloudflareClient::resolve_account_id(&token).await?,
    };

    let client = CloudflareClient::new(token, account, Some("flareprox".to_string()))?;
    let main_name = name_opt.unwrap_or_else(|| "frontlane-serp-edge".to_string());

    println!("\n  🗑  Tearing down Frontlane SERP Edge Stack...");
    println!("  ─────────────────────────────────────────────────────────────");

    // 1. Delete Main Worker
    print!("  Deleting Main Worker '{}'... ", main_name);
    match client.delete_worker(&main_name).await {
        Ok(true) => println!("✔ Deleted"),
        Ok(false) => println!("○ Not found"),
        Err(e) => println!("⚠ Error: {}", e),
    }

    // 2. Delete Proxy Workers
    println!("  Deleting FlareProx proxy lanes...");
    let cleaned = client.cleanup_all().await?;
    println!("  ✔ Deleted {} proxy worker(s)\n", cleaned);

    Ok(())
}
