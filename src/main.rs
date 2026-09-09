use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use frontlane_serp::cli::{
    AuditArgs, BatchRankArgs, Cli, CliFormat, Commands, ContactsArgs, CrawlArgs, ExtractArgs,
    FlareproxAction, FlareproxArgs, McpAction, RankArgs, SearchArgs, SuggestArgs,
};
use frontlane_serp::config::AppConfig;
use frontlane_serp::core::engine::SearchEngine;
use frontlane_serp::core::format::render_envelope;
use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::core::response_builder::enrich_result;
use frontlane_serp::core::types::{
    Envelope, OutputFormat, Pagination, Query, ResultItem, SerpFeature,
};
use frontlane_serp::engines::{
    Baidu, Bing, CratesIo, DuckDuckGo, Ecosia, GitHub, Google, HackerNews, Wikipedia, Yandex,
};
use frontlane_serp::extract::Extractor;
use frontlane_serp::mcp::run_stdio_mcp_server;
use frontlane_serp::mega::MegaSearcher;
use frontlane_serp::rank::{
    probe_engine_rank, DeviceType, DomainMatchMode, RankRequest, RankStrategy,
};
use frontlane_serp::server::run_server;
use frontlane_serp::suggest::SuggestClient;

fn to_output_format(fmt: CliFormat) -> OutputFormat {
    match fmt {
        CliFormat::Json => OutputFormat::Json,
        CliFormat::Csv => OutputFormat::Csv,
        CliFormat::Markdown => OutputFormat::Markdown,
        CliFormat::Text => OutputFormat::Text,
        CliFormat::Ndjson => OutputFormat::Ndjson,
    }
}

fn build_all_engines(http_client: &HttpClient) -> Vec<Arc<dyn SearchEngine>> {
    vec![
        Arc::new(Google::new(http_client.clone())),
        Arc::new(Bing::new(http_client.clone())),
        Arc::new(DuckDuckGo::new(http_client.clone())),
        Arc::new(Yandex::new(http_client.clone())),
        Arc::new(Baidu::new(http_client.clone())),
        Arc::new(Ecosia::new(http_client.clone())),
        Arc::new(HackerNews::new(http_client.clone())),
        Arc::new(GitHub::new(http_client.clone())),
        Arc::new(CratesIo::new(http_client.clone())),
        Arc::new(Wikipedia::new(http_client.clone())),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::from_level(
            log_level,
        ))
        .init();

    let mut config = AppConfig::load(&cli.config).unwrap_or_default();

    if let Some(ref p) = cli.proxy {
        config.proxies.global = Some(p.clone());
    }
    if let Some(ref h) = cli.host {
        config.server.host = h.clone();
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }

    let http_client = HttpClient::new(
        config.proxies.global.as_deref(),
        config.server.insecure,
        config.app.timeout,
    )?;
    let scraping_http_client = http_client.for_scraping(
        config.app.browser_impersonation,
        config.proxies.global.as_deref(),
        config.server.insecure,
        config.app.timeout,
    );

    let engines = build_all_engines(&scraping_http_client);

    match cli.command {
        Some(Commands::Serve(serve_args)) => {
            if let Some(ref h) = serve_args.host {
                config.server.host = h.clone();
            }
            if let Some(port) = serve_args.port {
                config.server.port = port;
            }
            run_server(config, engines, http_client).await?;
        }
        Some(Commands::Search(search_args)) => {
            handle_search(&search_args, &engines, &scraping_http_client).await?;
        }
        Some(Commands::Extract(extract_args)) => {
            handle_extract(&extract_args, &scraping_http_client).await?;
        }
        Some(Commands::Rank(rank_args)) => {
            handle_rank(&rank_args, &engines).await?;
        }
        Some(Commands::Suggest(suggest_args)) => {
            handle_suggest(&suggest_args, &http_client).await?;
        }
        Some(Commands::Crawl(crawl_args)) => {
            handle_crawl(&crawl_args, &scraping_http_client).await?;
        }
        Some(Commands::BatchRank(batch_args)) => {
            handle_batch_rank(&batch_args, &engines, &scraping_http_client).await?;
        }
        Some(Commands::Audit(audit_args)) => {
            handle_audit(&audit_args, &engines, &scraping_http_client).await?;
        }
        Some(Commands::Contacts(contacts_args)) => {
            handle_contacts(&contacts_args, &scraping_http_client).await?;
        }
        Some(Commands::Doctor(doctor_args)) => {
            let opts = frontlane_serp::cli::doctor::DoctorOptions {
                skip_engines: doctor_args.skip_engines,
                specific_engine: doctor_args.engine,
            };
            frontlane_serp::cli::doctor::run_doctor(&config, &engines, &scraping_http_client, opts)
                .await?;
        }
        Some(Commands::Mcp(mcp_args)) => match mcp_args.action {
            Some(McpAction::Install { client }) => {
                frontlane_serp::mcp::install_mcp(&client)?;
            }
            Some(McpAction::Status) => {
                frontlane_serp::mcp::status_mcp()?;
            }
            Some(McpAction::Uninstall { client }) => {
                frontlane_serp::mcp::uninstall_mcp(&client)?;
            }
            None | Some(McpAction::Serve) => {
                handle_mcp(&engines, &http_client, &scraping_http_client).await?;
            }
        },
        Some(Commands::Flareprox(flare_args)) => {
            handle_flareprox(&flare_args, &config).await?;
        }
        Some(Commands::Edge(edge_args)) => {
            handle_edge(&edge_args, &config).await?;
        }
        None => {
            // Default action: start server
            run_server(config, engines, http_client).await?;
        }
    }

    Ok(())
}

async fn handle_search(
    args: &SearchArgs,
    engines: &[Arc<dyn SearchEngine>],
    scraping_http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let norm_engine = match args.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        "hn" => "hackernews".to_string(),
        "gh" => "github".to_string(),
        "wiki" => "wikipedia".to_string(),
        n => n.to_string(),
    };

    let query = Query {
        text: args.query.clone(),
        lang_code: args.lang.clone(),
        region: args.region.clone(),
        date_interval: String::new(),
        filetype: String::new(),
        site: args.site.clone(),
        limit: args.limit,
        start: args.start,
        filter: true,
        features: true,
        extract: args.extract > 0,
        extract_top: args.extract,
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

    let format = to_output_format(args.format);
    let extractor = Extractor::new(scraping_http_client.clone());

    if norm_engine == "mega" {
        let mega = MegaSearcher::new(engines.to_vec(), Some(extractor));
        let engines_req = args
            .engines
            .as_deref()
            .map(|s| s.split(',').map(|e| e.trim().to_string()).collect())
            .unwrap_or_else(|| {
                vec![
                    "google".to_string(),
                    "bing".to_string(),
                    "duckduckgo".to_string(),
                ]
            });

        let env = mega.search(&query, &engines_req, &args.mode).await?;
        println!("{}", render_envelope(&env, format));
    } else {
        let engine = engines
            .iter()
            .find(|e| e.name().eq_ignore_ascii_case(&norm_engine))
            .ok_or_else(|| format!("Unknown engine: {}", args.engine))?;

        let started_at = chrono::Utc::now();
        let start_time = std::time::Instant::now();
        let results = engine.search(&query).await?;
        let took_ms = start_time.elapsed().as_millis() as i64;

        let mut env = Envelope::new(
            &query,
            uuid::Uuid::now_v7().to_string(),
            started_at,
            vec![norm_engine.clone()],
        );
        env.meta.took_ms = took_ms;
        env.meta.engines_responded = vec![norm_engine.clone()];

        let mut enriched_results: Vec<ResultItem> = Vec::new();
        let mut serp_features: Vec<SerpFeature> = Vec::new();

        for raw in results {
            for feat in &raw.features {
                serp_features.push(feat.clone());
            }
            let enriched = enrich_result(raw, &norm_engine, query.start);
            enriched_results.push(enriched);
        }

        if query.extract {
            let extract_count = query.extract_top.min(enriched_results.len());
            for item in enriched_results.iter_mut().take(extract_count) {
                if let Ok(content) = extractor.extract(&item.url, true).await {
                    item.extracted = Some(content);
                }
            }
        }

        env.results = enriched_results;
        env.serp_features = serp_features;
        env.pagination = Pagination {
            page: (query.start / 10) + 1,
            has_more: env.results.len() >= query.limit,
            next_start: query.start + query.limit,
        };

        println!("{}", render_envelope(&env, format));
    }

    Ok(())
}

async fn handle_extract(
    args: &ExtractArgs,
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let extractor = Extractor::new(http_client.clone());
    let content = extractor.extract(&args.url, args.llms_txt).await?;

    let format = to_output_format(args.format);
    match format {
        OutputFormat::Markdown | OutputFormat::Text => {
            if let Some(text) = content.content {
                println!("{}", text);
            } else if let Some(err) = content.error {
                eprintln!("Extraction error: {}", err);
            }
        }
        _ => {
            println!("{}", serde_json::to_string_pretty(&content)?);
        }
    }

    Ok(())
}

async fn handle_rank(
    args: &RankArgs,
    engines: &[Arc<dyn SearchEngine>],
) -> Result<(), Box<dyn std::error::Error>> {
    let norm_engine = match args.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = engines
        .iter()
        .find(|e| e.name().eq_ignore_ascii_case(&norm_engine))
        .ok_or_else(|| format!("Unknown engine: {}", args.engine))?
        .clone();

    let strategy = match args.strategy.as_str() {
        "basic" => RankStrategy::Basic,
        "custom" => RankStrategy::Custom,
        _ => RankStrategy::Smart,
    };

    let match_mode = match args.r#match.as_str() {
        "exact" => DomainMatchMode::Exact,
        "wildcard" => DomainMatchMode::Wildcard,
        "directory" => DomainMatchMode::Directory,
        _ => DomainMatchMode::Subdomain,
    };

    let device = match args.device.as_str() {
        "mobile" => DeviceType::Mobile,
        _ => DeviceType::Desktop,
    };

    let req = RankRequest {
        target: args.target.clone(),
        q: args.query.clone(),
        strategy,
        last_rank: args.last_rank,
        pagination_limit: args.limit,
        smart_full_fallback: args.fallback,
        r#match: match_mode,
        device,
        region: "US".to_string(),
        lang: "en".to_string(),
    };

    let resp = probe_engine_rank(engine, &req)
        .await
        .map_err(|e| format!("{e}"))?;
    if args.format == CliFormat::Json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        if resp.ranked {
            println!(
                "🎯 RANK #{} | Target: {} | Engine: {} | URL: {}",
                resp.rank.unwrap_or(0),
                resp.target,
                resp.engine,
                resp.url.as_deref().unwrap_or("")
            );
        } else {
            println!(
                "❌ NOT RANKED | Target: {} | Checked {} pages | Engine: {}",
                resp.target,
                resp.pages_scraped.len(),
                resp.engine
            );
        }

        if resp.directory_count > 0 || args.directories {
            let top_dirs = if resp.ranking_directories.is_empty() {
                "None".to_string()
            } else {
                resp.ranking_directories.join(", ")
            };
            println!(
                "📁 Directory Dominance: {}/{} results ({:.1}%) — {}",
                resp.directory_count,
                resp.serp_results.len(),
                resp.directory_share_pct,
                top_dirs
            );
        }

        if !resp.serp_results.is_empty() {
            println!("\nTop SERP Results ({}):", resp.serp_results.len());
            for res in &resp.serp_results {
                let badge = if res.is_target { " 🎯 [TARGET]" } else { "" };
                let dir_badge = if res.is_directory {
                    " 📁 [DIRECTORY]"
                } else {
                    ""
                };
                let domain_str = res
                    .domain
                    .as_deref()
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default();
                println!(
                    "  #{}: {} - {}{}{}{}",
                    res.rank, res.url, res.title, domain_str, badge, dir_badge
                );
            }
        }
    }

    Ok(())
}

async fn handle_audit(
    args: &AuditArgs,
    engines: &[Arc<dyn SearchEngine>],
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let norm_engine = match args.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = engines
        .iter()
        .find(|e| e.name().eq_ignore_ascii_case(&norm_engine))
        .ok_or_else(|| format!("Unknown engine: {}", args.engine))?
        .clone();

    eprintln!(
        "🔍 Running SERP competitor audit for '{}' on target '{}' using {}...",
        args.query, args.target, norm_engine
    );

    let report = frontlane_serp::audit::run_audit(
        engine,
        http_client,
        &args.target,
        args.target_url.as_deref(),
        &args.query,
        args.limit,
    )
    .await
    .map_err(|e| format!("{e}"))?;

    let output_str = match args.format {
        CliFormat::Json => report.to_json()?,
        CliFormat::Csv => report.to_csv(),
        CliFormat::Markdown => report.to_markdown(),
        CliFormat::Text | CliFormat::Ndjson => report.to_console_text(),
    };

    if let Some(ref out_path) = args.output {
        let content_to_write = if out_path.ends_with(".csv") {
            report.to_csv()
        } else if out_path.ends_with(".md") {
            report.to_markdown()
        } else {
            report.to_json()?
        };
        tokio::fs::write(out_path, content_to_write).await?;
        eprintln!("✅ Saved audit report to '{}'.", out_path);
    }

    println!("{}", output_str);

    Ok(())
}

async fn handle_contacts(
    args: &ContactsArgs,
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "📞 Scanning contacts for '{}' (crawl: {}, max_pages: {})...",
        args.url, args.crawl, args.max_pages
    );

    let contacts = frontlane_serp::extract::scan_contacts(
        http_client,
        &args.url,
        args.crawl,
        args.max_pages,
        args.max_depth,
    )
    .await
    .map_err(|e| format!("{e}"))?;

    let output_str = match args.format {
        CliFormat::Json => serde_json::to_string_pretty(&contacts)?,
        CliFormat::Csv => contacts.to_csv(),
        CliFormat::Markdown => contacts.to_markdown(),
        CliFormat::Text | CliFormat::Ndjson => contacts.to_console_text(),
    };

    if let Some(ref out_path) = args.output {
        let content_to_write = if out_path.ends_with(".csv") {
            contacts.to_csv()
        } else if out_path.ends_with(".md") {
            contacts.to_markdown()
        } else {
            serde_json::to_string_pretty(&contacts)?
        };
        tokio::fs::write(out_path, content_to_write).await?;
        eprintln!("✅ Saved contacts report to '{}'.", out_path);
    }

    println!("{}", output_str);

    Ok(())
}

async fn handle_suggest(
    args: &SuggestArgs,
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = SuggestClient::new(http_client.clone());
    let resp = client
        .suggest(&args.engine, &args.query, &args.lang, &args.region)
        .await
        .map_err(|e| format!("{e}"))?;

    if args.format == CliFormat::Json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("Suggestions for '{}' ({}):", resp.query, resp.engine);
        for (i, s) in resp.suggestions.iter().enumerate() {
            println!("  {}. {}", i + 1, s);
        }
    }

    Ok(())
}

async fn handle_mcp(
    engines: &[Arc<dyn SearchEngine>],
    http_client: &HttpClient,
    scraping_http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine_map = std::collections::HashMap::new();
    for e in engines {
        engine_map.insert(e.name().to_string(), e.clone());
    }
    let extractor = Arc::new(Extractor::new(scraping_http_client.clone()));
    let mega = Arc::new(MegaSearcher::new(
        engines.to_vec(),
        Some((*extractor).clone()),
    ));
    let suggest = SuggestClient::new(http_client.clone());
    let crawler = Arc::new(frontlane_serp::crawl::Crawler::new(
        scraping_http_client.clone(),
        extractor.clone(),
    ));

    run_stdio_mcp_server(engine_map, mega, extractor, suggest, crawler).await?;
    Ok(())
}

async fn handle_crawl(
    args: &CrawlArgs,
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let extractor = Arc::new(Extractor::new(http_client.clone()));
    let crawler = frontlane_serp::crawl::Crawler::new(http_client.clone(), extractor);

    let options = frontlane_serp::crawl::CrawlOptions {
        start_url: args.url.clone(),
        max_depth: args.max_depth,
        max_pages: args.max_pages,
        concurrency: 4,
        allowed_domains: vec![],
        respect_robots: args.respect_robots,
        extract_content: args.extract,
    };

    let result = crawler.crawl(options).await?;

    if args.format == CliFormat::Json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Crawled: {} ({} pages, {}ms)\n",
            result.start_url, result.pages_crawled, result.took_ms
        );
        for (i, page) in result.pages.iter().enumerate() {
            println!(
                "[{}] {} (depth {}, status {})",
                i + 1,
                page.url,
                page.depth,
                page.status
            );
            if let Some(ref title) = page.title {
                println!("    Title: {}", title);
            }
            println!("    Links found: {}", page.links.len());
            if !page.json_ld.is_empty() {
                println!("    JSON-LD items: {}", page.json_ld.len());
            }
            if !page.meta_tags.is_empty() {
                println!("    Meta tags: {}", page.meta_tags.len());
            }
            if let Some(ref content) = page.content {
                let snippet: String = content.chars().take(120).collect();
                println!("    Content: {}...", snippet.trim().replace('\n', " "));
            }
            println!();
        }
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct BatchKeywordItem {
    keyword: String,
    #[serde(default)]
    target_url: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    target_geo: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct BatchRankResultItem {
    keyword: String,
    category: Option<String>,
    priority: Option<String>,
    target_geo: Option<String>,
    target_url: Option<String>,
    ranked: bool,
    rank: Option<usize>,
    serp_url: Option<String>,
    title: Option<String>,
    pages_scraped: usize,
    took_ms: u64,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default)]
    directory_count: usize,
    #[serde(default)]
    directory_share_pct: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    top_directories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    serp_results: Vec<frontlane_serp::rank::SerpRankResultItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audit: Option<frontlane_serp::audit::KeywordAuditReport>,
}

async fn handle_batch_rank(
    args: &BatchRankArgs,
    engines: &[Arc<dyn SearchEngine>],
    http_client: &HttpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let norm_engine = match args.engine.to_lowercase().as_str() {
        "ddg" | "duck" => "duckduckgo".to_string(),
        n => n.to_string(),
    };

    let engine = engines
        .iter()
        .find(|e| e.name().eq_ignore_ascii_case(&norm_engine))
        .ok_or_else(|| format!("Unknown engine: {}", args.engine))?
        .clone();

    let file_content = tokio::fs::read_to_string(&args.file)
        .await
        .map_err(|e| format!("Failed to read keywords file '{}': {}", args.file, e))?;

    let mut items: Vec<BatchKeywordItem> = Vec::new();

    if args.file.ends_with(".csv") {
        let lines: Vec<&str> = file_content.lines().collect();
        if !lines.is_empty() {
            let header: Vec<&str> = lines[0].split(',').map(|s| s.trim()).collect();
            let kw_idx = header
                .iter()
                .position(|&h| h.eq_ignore_ascii_case("keyword"))
                .unwrap_or(0);
            let url_idx = header
                .iter()
                .position(|&h| h.eq_ignore_ascii_case("target_url"));
            let cat_idx = header
                .iter()
                .position(|&h| h.eq_ignore_ascii_case("category"));
            let prio_idx = header
                .iter()
                .position(|&h| h.eq_ignore_ascii_case("priority"));
            let geo_idx = header
                .iter()
                .position(|&h| h.eq_ignore_ascii_case("target_geo"));

            for line in &lines[1..] {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let cols: Vec<&str> = trimmed
                    .split(',')
                    .map(|c| c.trim().trim_matches('"'))
                    .collect();
                if cols.len() > kw_idx && !cols[kw_idx].is_empty() {
                    items.push(BatchKeywordItem {
                        keyword: cols[kw_idx].to_string(),
                        target_url: url_idx.and_then(|i| cols.get(i).map(|s| s.to_string())),
                        category: cat_idx.and_then(|i| cols.get(i).map(|s| s.to_string())),
                        priority: prio_idx.and_then(|i| cols.get(i).map(|s| s.to_string())),
                        target_geo: geo_idx.and_then(|i| cols.get(i).map(|s| s.to_string())),
                    });
                }
            }
        }
    } else if args.file.ends_with(".json") {
        items = serde_json::from_str(&file_content)
            .map_err(|e| format!("Failed to parse JSON keyword list: {}", e))?;
    } else {
        // Plain text line-separated
        for line in file_content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                items.push(BatchKeywordItem {
                    keyword: trimmed.to_string(),
                    target_url: None,
                    category: None,
                    priority: None,
                    target_geo: None,
                });
            }
        }
    }

    let mut results: Vec<BatchRankResultItem> = Vec::new();
    let mut already_processed: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(ref out_path) = args.output {
        if (args.resume || args.offset > 0) && tokio::fs::metadata(out_path).await.is_ok() {
            if let Ok(content) = tokio::fs::read_to_string(out_path).await {
                if let Ok(loaded) = serde_json::from_str::<Vec<BatchRankResultItem>>(&content) {
                    for it in &loaded {
                        already_processed.insert(it.keyword.clone());
                    }
                    results = loaded;
                    eprintln!(
                        "📋 Loaded {} existing results from '{}'.",
                        results.len(),
                        out_path
                    );
                }
            }
        }
    }

    if args.resume {
        items.retain(|it| !already_processed.contains(&it.keyword));
    } else if args.offset > 0 {
        if args.offset < items.len() {
            items = items.split_off(args.offset);
        } else {
            items.clear();
        }
    }

    if let Some(max) = args.max {
        items.truncate(max);
    }

    let total = items.len();
    if total == 0 {
        eprintln!("No keywords to process (already finished or empty).");
        return Ok(());
    }

    eprintln!(
        "🚀 Starting batch rank tracking for {} keywords on target '{}' using engine '{}'...",
        total, args.target, norm_engine
    );

    for (idx, item) in items.into_iter().enumerate() {
        let count = idx + 1;
        let cat_label = item.category.as_deref().unwrap_or("General");
        eprint!(
            "[{}/{}] '{}' ({}) ... ",
            count, total, item.keyword, cat_label
        );

        let match_mode = match args.r#match.as_str() {
            "exact" => DomainMatchMode::Exact,
            "wildcard" => DomainMatchMode::Wildcard,
            "directory" => DomainMatchMode::Directory,
            _ => DomainMatchMode::Subdomain,
        };

        let req = RankRequest {
            target: args.target.clone(),
            q: item.keyword.clone(),
            strategy: RankStrategy::Smart,
            last_rank: 0,
            pagination_limit: args.limit,
            smart_full_fallback: false,
            r#match: match_mode,
            device: DeviceType::Desktop,
            region: "US".to_string(),
            lang: "en".to_string(),
        };

        let mut retries = 0;
        let max_retries = 3;

        loop {
            let start_t = std::time::Instant::now();
            let rank_res = probe_engine_rank(engine.clone(), &req).await;
            let elapsed = start_t.elapsed();

            match rank_res {
                Ok(resp) => {
                    let rank_str = if resp.ranked {
                        format!("🎯 Rank #{}", resp.rank.unwrap_or(0))
                    } else {
                        "❌ Unranked".to_string()
                    };

                    let top_preview = if !resp.serp_results.is_empty() {
                        let top_domains: Vec<&str> = resp
                            .serp_results
                            .iter()
                            .take(3)
                            .filter_map(|r| r.domain.as_deref())
                            .collect();
                        if !top_domains.is_empty() {
                            format!(" [Top: {}]", top_domains.join(", "))
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    let dir_preview = if resp.directory_count > 0 {
                        format!(
                            " [📁 Dirs: {} ({:.0}%)]",
                            resp.directory_count, resp.directory_share_pct
                        )
                    } else {
                        String::new()
                    };

                    eprintln!(
                        "{}{}{}({}ms)",
                        rank_str,
                        top_preview,
                        dir_preview,
                        elapsed.as_millis()
                    );

                    let audit_data = if args.audit {
                        eprint!(" [auditing top competitors...] ");
                        frontlane_serp::audit::run_audit_for_rank_response(
                            engine.clone(),
                            http_client,
                            &args.target,
                            item.target_url.as_deref(),
                            &item.keyword,
                            &resp,
                            args.top_results.clamp(1, 3),
                        )
                        .await
                        .ok()
                    } else {
                        None
                    };

                    let serp_slice = if args.top_results > 0 {
                        resp.serp_results
                            .into_iter()
                            .take(args.top_results)
                            .collect()
                    } else {
                        Vec::new()
                    };

                    results.push(BatchRankResultItem {
                        keyword: item.keyword.clone(),
                        category: item.category.clone(),
                        priority: item.priority.clone(),
                        target_geo: item.target_geo.clone(),
                        target_url: item.target_url.clone(),
                        ranked: resp.ranked,
                        rank: resp.rank,
                        serp_url: resp.url,
                        title: resp.title,
                        pages_scraped: resp.pages_scraped.len(),
                        took_ms: elapsed.as_millis() as u64,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        error: None,
                        directory_count: resp.directory_count,
                        directory_share_pct: resp.directory_share_pct,
                        top_directories: resp.ranking_directories.clone(),
                        serp_results: serp_slice,
                        audit: audit_data,
                    });
                    break;
                }
                Err(e) => {
                    let err_msg = format!("{e}");
                    retries += 1;
                    if retries <= max_retries
                        && (err_msg.contains("captcha")
                            || err_msg.contains("Blocked")
                            || err_msg.contains("Forbidden")
                            || err_msg.contains("reset"))
                    {
                        let backoff_secs = 5 * retries;
                        eprintln!("⏳ Rate limited ({err_msg}). Backing off for {backoff_secs}s before retry {retries}/{max_retries}...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs as u64))
                            .await;
                    } else {
                        eprintln!("⚠️  Error: {}", err_msg);
                        results.push(BatchRankResultItem {
                            keyword: item.keyword.clone(),
                            category: item.category.clone(),
                            priority: item.priority.clone(),
                            target_geo: item.target_geo.clone(),
                            target_url: item.target_url.clone(),
                            ranked: false,
                            rank: None,
                            serp_url: None,
                            title: None,
                            pages_scraped: 0,
                            took_ms: elapsed.as_millis() as u64,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            error: Some(err_msg),
                            directory_count: 0,
                            directory_share_pct: 0.0,
                            top_directories: Vec::new(),
                            serp_results: Vec::new(),
                            audit: None,
                        });
                        break;
                    }
                }
            }
        }

        // Periodically write intermediate progress every 5 queries
        if let Some(ref out_path) = args.output {
            if count % 5 == 0 || count == total {
                let inter_content = if out_path.ends_with(".csv") {
                    batch_results_to_csv(&results)
                } else if out_path.ends_with(".md") {
                    batch_results_to_markdown(&results)
                } else {
                    serde_json::to_string_pretty(&results).unwrap_or_default()
                };
                let _ = tokio::fs::write(out_path, inter_content).await;
            }
        }

        if args.delay_ms > 0 && count < total {
            tokio::time::sleep(tokio::time::Duration::from_millis(args.delay_ms)).await;
        }
    }

    if let Some(ref out_path) = args.output {
        let content_to_write = if out_path.ends_with(".csv") {
            batch_results_to_csv(&results)
        } else if out_path.ends_with(".md") {
            batch_results_to_markdown(&results)
        } else {
            serde_json::to_string_pretty(&results)?
        };
        tokio::fs::write(out_path, content_to_write).await?;
        eprintln!("\n✅ Saved {} results to '{}'.", results.len(), out_path);
    }

    // Aggregate Directory Intelligence Summary across the batch
    let mut total_serp_positions = 0;
    let mut total_directory_positions = 0;
    let mut dir_domain_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for r in &results {
        for s in &r.serp_results {
            total_serp_positions += 1;
            if s.is_directory {
                total_directory_positions += 1;
                let d = s.domain.clone().unwrap_or_else(|| "unknown".to_string());
                *dir_domain_counts.entry(d).or_insert(0) += 1;
            }
        }
    }

    if total_directory_positions > 0 || args.directories {
        let mut sorted_dirs: Vec<(String, usize)> = dir_domain_counts.into_iter().collect();
        sorted_dirs.sort_by_key(|a| std::cmp::Reverse(a.1));
        let share_pct = if total_serp_positions > 0 {
            (total_directory_positions as f64 / total_serp_positions as f64) * 100.0
        } else {
            0.0
        };

        eprintln!(
            "\n📁 Directory Intelligence & Market Share across {} keywords:",
            results.len()
        );
        eprintln!(
            "  • Directory SERP Share: {}/{} total positions ({:.1}%)",
            total_directory_positions, total_serp_positions, share_pct
        );
        if !sorted_dirs.is_empty() {
            eprintln!("  • Top Dominating Directories:");
            for (dir, cnt) in sorted_dirs.iter().take(8) {
                let pct = (*cnt as f64 / total_directory_positions as f64) * 100.0;
                eprintln!("    - {:<22} : {} positions ({:.1}%)", dir, cnt, pct);
            }
        }
    }

    match args.format {
        CliFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
        CliFormat::Csv => println!("{}", batch_results_to_csv(&results)),
        CliFormat::Markdown => println!("{}", batch_results_to_markdown(&results)),
        CliFormat::Text | CliFormat::Ndjson => {}
    }

    Ok(())
}

fn batch_results_to_csv(results: &[BatchRankResultItem]) -> String {
    let mut out = String::new();
    out.push_str("keyword,category,priority,target_geo,target_url,ranked,rank,found_url,title,took_ms,directory_count,directory_share_pct,ranking_directories,top_competitor_1,top_competitor_2,top_competitor_3,key_recommendation\n");
    for item in results {
        let comp1 = item
            .serp_results
            .first()
            .map(|r| r.url.as_str())
            .unwrap_or("");
        let comp2 = item
            .serp_results
            .get(1)
            .map(|r| r.url.as_str())
            .unwrap_or("");
        let comp3 = item
            .serp_results
            .get(2)
            .map(|r| r.url.as_str())
            .unwrap_or("");
        let rec = item
            .audit
            .as_ref()
            .and_then(|a| a.insights.first())
            .map(|i| format!("[{}] {}", i.category, i.actionable_recommendation))
            .unwrap_or_default();
        out.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},\"{}\",\"{}\",\"{}\",{},{},{:.1},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
            escape_csv_field(&item.keyword),
            escape_csv_field(item.category.as_deref().unwrap_or("")),
            escape_csv_field(item.priority.as_deref().unwrap_or("")),
            escape_csv_field(item.target_geo.as_deref().unwrap_or("")),
            escape_csv_field(item.target_url.as_deref().unwrap_or("")),
            item.ranked,
            item.rank.map(|r| r.to_string()).unwrap_or_default(),
            escape_csv_field(item.serp_url.as_deref().unwrap_or("")),
            escape_csv_field(item.title.as_deref().unwrap_or("")),
            item.took_ms,
            item.directory_count,
            item.directory_share_pct,
            escape_csv_field(&item.top_directories.join("; ")),
            escape_csv_field(comp1),
            escape_csv_field(comp2),
            escape_csv_field(comp3),
            escape_csv_field(&rec),
        ));
    }
    out
}

fn batch_results_to_markdown(results: &[BatchRankResultItem]) -> String {
    let mut out = String::new();
    out.push_str("# Batch Rank Tracking & Competitor Comparison Report\n\n");
    out.push_str(
        "| Keyword | Category | Rank | URL | Directories in SERP | Top Competitors | Primary Recommendation |\n",
    );
    out.push_str("| :--- | :--- | :---: | :--- | :--- | :--- | :--- |\n");
    for item in results {
        let rank_badge = if item.ranked {
            format!("**#{}**", item.rank.unwrap_or(0))
        } else {
            "❌ Unranked".to_string()
        };
        let top_comp = item
            .serp_results
            .iter()
            .take(2)
            .filter_map(|r| r.domain.as_deref())
            .collect::<Vec<_>>()
            .join(", ");
        let dir_str = if item.directory_count > 0 {
            format!(
                "{}/{} ({:.0}%)",
                item.directory_count,
                item.serp_results.len(),
                item.directory_share_pct
            )
        } else {
            "—".to_string()
        };
        let rec = item
            .audit
            .as_ref()
            .and_then(|a| a.insights.first())
            .map(|i| format!("**{}**: {}", i.category, i.actionable_recommendation))
            .unwrap_or_else(|| "—".to_string());
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            item.keyword,
            item.category.as_deref().unwrap_or("—"),
            rank_badge,
            item.serp_url.as_deref().unwrap_or("—"),
            dir_str,
            if top_comp.is_empty() {
                "—"
            } else {
                &top_comp
            },
            rec
        ));
    }
    out
}

fn escape_csv_field(s: &str) -> String {
    s.replace('"', "\"\"").replace('\n', " ").replace('\r', "")
}

async fn handle_flareprox(
    args: &FlareproxArgs,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = args
        .token
        .clone()
        .or_else(|| config.flareprox.api_token.clone())
        .or_else(|| std::env::var("CLOUDFLARE_API_TOKEN").ok())
        .or_else(|| std::env::var("CF_API_TOKEN").ok())
        .or_else(frontlane_serp::flareprox::detect_wrangler_token)
        .filter(|s| !s.trim().is_empty())
        .ok_or(
            "Cloudflare API Token required. Pass --token, set in config.yaml under flareprox.api_token, export CLOUDFLARE_API_TOKEN, or log in with wrangler.",
        )?;

    let account = match args
        .account
        .clone()
        .or_else(|| config.flareprox.account_id.clone())
        .or_else(|| std::env::var("CLOUDFLARE_ACCOUNT_ID").ok())
        .or_else(|| std::env::var("CF_ACCOUNT_ID").ok())
        .filter(|s| !s.trim().is_empty())
    {
        Some(acc) => acc,
        None => {
            println!("🔍 Auto-detecting Cloudflare Account ID from token...");
            match frontlane_serp::flareprox::CloudflareClient::resolve_account_id(&token).await {
                Ok(acc) => {
                    println!("  Using account ID: {}\n", acc);
                    acc
                }
                Err(e) => {
                    return Err(format!(
                        "Cloudflare Account ID required: {}. Pass --account or set in config.yaml under flareprox.account_id.",
                        e
                    )
                    .into());
                }
            }
        }
    };

    let prefix = args
        .prefix
        .clone()
        .unwrap_or_else(|| config.flareprox.worker_prefix.clone());

    let client = frontlane_serp::flareprox::CloudflareClient::new(token, account, Some(prefix))?;

    match &args.action {
        FlareproxAction::Create {
            count,
            name,
            region,
        } => {
            let n = (*count).max(1);
            let region_label = region.as_deref().unwrap_or("default");
            println!(
                "🔥 Deploying {} FlareProx worker proxy endpoint(s) in region '{}'...",
                n, region_label
            );

            let mut deployed = Vec::new();
            for i in 0..n {
                let worker_name = if n == 1 { name.as_deref() } else { None };
                print!("  [{}/{}] Deploying worker... ", i + 1, n);
                match client.create_worker(worker_name, region.as_deref()).await {
                    Ok(dep) => {
                        println!("✅ {}", dep.url);
                        deployed.push(dep);
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                    }
                }
            }

            println!("\n✨ Deployed {} endpoint(s).", deployed.len());
            if !deployed.is_empty() {
                println!("\nTest your worker proxy directly:");
                println!(
                    "  curl -H 'X-Target-URL: https://ifconfig.me/ip' {}",
                    deployed[0].url
                );
                println!("  curl '{}/?url=https://ifconfig.me/ip'", deployed[0].url);
                println!("\nUse in Frontlane SERP queries:");
                println!(
                    "  frontlane-serp search google \"query\" --proxy {}",
                    deployed[0].url
                );
            }
        }
        FlareproxAction::List => {
            println!("🔍 Fetching deployed FlareProx workers from Cloudflare...");
            let workers = client.list_workers().await?;
            if workers.is_empty() {
                println!(
                    "  No deployed FlareProx workers found matching prefix '{}'.",
                    client.worker_prefix()
                );
                println!("  Run 'frontlane-serp flareprox create' to deploy one.");
            } else {
                println!("\n  Active FlareProx Worker Endpoints ({})", workers.len());
                println!("  {:<35} {:<55} {:<24}", "NAME", "URL", "CREATED AT");
                println!("  {}", "-".repeat(115));
                for w in &workers {
                    println!("  {:<35} {:<55} {:<24}", w.name, w.url, w.created_at);
                }
            }
        }
        FlareproxAction::Test { target } => {
            println!("🔍 Testing FlareProx endpoints against: {}", target);
            let workers = client.list_workers().await?;
            if workers.is_empty() {
                println!("  No active FlareProx workers found to test.");
                return Ok(());
            }

            println!(
                "  Found {} worker(s). Testing connectivity and egress IPs...\n",
                workers.len()
            );
            println!(
                "  {:<35} {:<8} {:<10} {:<18} {:<10}",
                "NAME", "STATUS", "LATENCY", "EGRESS IP", "RESULT"
            );
            println!("  {}", "-".repeat(90));

            for w in workers {
                let res = client.test_endpoint(&w, target).await;
                let status_str = if res.status > 0 {
                    res.status.to_string()
                } else {
                    "ERR".to_string()
                };
                let lat_str = format!("{}ms", res.latency_ms);
                let ip_str = res.egress_ip.as_deref().unwrap_or("-");
                let result_str = if res.success { "✅ OK" } else { "❌ FAILED" };

                println!(
                    "  {:<35} {:<8} {:<10} {:<18} {:<10}",
                    res.name, status_str, lat_str, ip_str, result_str
                );
            }
        }
        FlareproxAction::Delete { names } => {
            if names.is_empty() {
                println!("Please specify at least one worker name to delete.");
                return Ok(());
            }
            for name in names {
                print!("Deleting worker '{}'... ", name);
                match client.delete_worker(name).await {
                    Ok(true) => println!("✅ Deleted"),
                    Ok(false) => println!("⚠️ Not found"),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
        }
        FlareproxAction::Cleanup => {
            println!(
                "🧹 Cleaning up all FlareProx workers matching prefix '{}'...",
                client.worker_prefix()
            );
            let count = client.cleanup_all().await?;
            println!("✅ Successfully deleted {} FlareProx worker(s).", count);
        }
        FlareproxAction::Sync {
            config: config_path,
        } => {
            println!(
                "🔄 Syncing active FlareProx workers into '{}'...",
                config_path
            );
            let workers = client.list_workers().await?;
            if workers.is_empty() {
                println!("  No active FlareProx workers found on Cloudflare account.");
                return Ok(());
            }

            let mut updated_config = AppConfig::load(config_path).unwrap_or_default();
            updated_config.flareprox.workers = workers.iter().map(|w| w.url.clone()).collect();
            updated_config.flareprox.enabled = true;

            for w in &workers {
                let already_exists = updated_config
                    .proxies
                    .entries
                    .iter()
                    .any(|e| e.url == w.url);
                if !already_exists {
                    updated_config
                        .proxies
                        .entries
                        .push(frontlane_serp::config::ProxyEntry {
                            url: w.url.clone(),
                            tags: vec![
                                "flareprox".to_string(),
                                "cf".to_string(),
                                "rotating".to_string(),
                            ],
                        });
                }
            }

            let serialized = serde_yaml::to_string(&updated_config)?;
            tokio::fs::write(config_path, serialized).await?;
            println!(
                "✅ Synced {} worker(s) to '{}' under proxy pool.",
                workers.len(),
                config_path
            );
        }
    }

    Ok(())
}

async fn handle_edge(
    args: &frontlane_serp::cli::EdgeArgs,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match &args.action {
        frontlane_serp::cli::EdgeAction::Deploy {
            name,
            proxies,
            region,
            auto_recycle,
        } => {
            let opts = frontlane_serp::cli::edge::EdgeDeployOptions {
                name: Some(name.clone()),
                proxies: *proxies,
                region: Some(region.clone()),
                auto_recycle: *auto_recycle,
            };
            frontlane_serp::cli::edge::run_edge_deploy(
                config,
                args.token.clone(),
                args.account.clone(),
                opts,
            )
            .await?;
        }
        frontlane_serp::cli::EdgeAction::Status { name } => {
            frontlane_serp::cli::edge::run_edge_status(
                config,
                args.token.clone(),
                args.account.clone(),
                Some(name.clone()),
            )
            .await?;
        }
        frontlane_serp::cli::EdgeAction::Destroy { name } => {
            frontlane_serp::cli::edge::run_edge_destroy(
                config,
                args.token.clone(),
                args.account.clone(),
                Some(name.clone()),
            )
            .await?;
        }
    }
    Ok(())
}
