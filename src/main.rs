use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use frontlane_serp::cli::{
    Cli, CliFormat, Commands, CrawlArgs, ExtractArgs, RankArgs, SearchArgs, SuggestArgs,
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
    let engines = build_all_engines(&http_client);

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
            handle_search(&search_args, &engines, &http_client).await?;
        }
        Some(Commands::Extract(extract_args)) => {
            handle_extract(&extract_args, &http_client).await?;
        }
        Some(Commands::Rank(rank_args)) => {
            handle_rank(&rank_args, &engines).await?;
        }
        Some(Commands::Suggest(suggest_args)) => {
            handle_suggest(&suggest_args, &http_client).await?;
        }
        Some(Commands::Crawl(crawl_args)) => {
            handle_crawl(&crawl_args, &http_client).await?;
        }
        Some(Commands::Mcp) => {
            handle_mcp(&engines, &http_client).await?;
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
    http_client: &HttpClient,
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
    let extractor = Extractor::new(http_client.clone());

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
    } else if resp.ranked {
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
) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine_map = std::collections::HashMap::new();
    for e in engines {
        engine_map.insert(e.name().to_string(), e.clone());
    }
    let extractor = Arc::new(Extractor::new(http_client.clone()));
    let mega = Arc::new(MegaSearcher::new(
        engines.to_vec(),
        Some((*extractor).clone()),
    ));
    let suggest = SuggestClient::new(http_client.clone());
    let crawler = Arc::new(frontlane_serp::crawl::Crawler::new(
        http_client.clone(),
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
