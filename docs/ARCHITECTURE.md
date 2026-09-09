# Frontlane SERP Architecture

## Overview

Frontlane SERP is a high-performance, asynchronous SERP API, CLI, and Model Context Protocol (MCP) server written in Rust. It queries search engines (Google, Bing, DuckDuckGo, Yandex, Baidu, Ecosia, Hacker News, GitHub, Crates.io, and Wikipedia) via resilient HTTP clients with TLS fingerprinting, robust anti-bot bypass, structured metadata extraction, and rank probing.

Execution characteristics:
- **Zero Headless Browser Overhead**: Fast, direct async HTTP fetching with custom TLS, decompression (gzip/brotli), cookie jar management, and user-agent rotation.
- **Async I/O**: Driven by Tokio multi-threaded runtime with Axum 0.8 as the HTTP server layer.
- **Safe Concurrency**: Bounded queues, async semaphore rate-limiters, and zero data races.

## Project Layout

```text
frontlane-serp/
├── Cargo.toml           # Rust package manifest & dependencies
├── Dockerfile           # Multi-stage release container build
├── src/
│   ├── lib.rs           # Library entrypoint & module definitions
│   ├── main.rs          # Binary CLI entrypoint (Clap dispatcher)
│   ├── cli/             # CLI commands: search, rank, suggest, crawl, mcp, serve
│   ├── compat/          # Serper & SerpApi drop-in compatibility handlers
│   ├── config.rs        # YAML configuration loader & serde data structures
│   ├── core/            # Core primitives:
│   │   ├── engine.rs    # SearchEngine trait definition
│   │   ├── http_client.rs # Reqwest HTTP client with proxy & retry support
│   │   ├── proxy.rs     # Proxy pools, geo-routing, and cooldowns
│   │   ├── cache.rs     # Moka async memory cache
│   │   ├── circuit_breaker.rs # Three-state circuit breaker
│   │   ├── network_guard.rs # SSRF protection and private IP blocking
│   │   ├── clusters.rs  # Multi-engine consensus & cross-engine clustering
│   │   ├── domain.rs    # TLD/SLD domain parsing & classification
│   │   ├── format.rs    # Markdown, JSON, NDJSON, Text formatters
│   │   └── types.rs     # Common request/response envelopes and models
│   ├── crawl/           # Domain-restricted BFS crawler & robots.txt parser
│   ├── engines/         # Engine implementations:
│   │   ├── google/      # Google HTML parser & SERP feature extractor
│   │   ├── bing/        # Bing HTML parser & feature extractor
│   │   ├── duckduckgo/  # DuckDuckGo HTML parser & feature extractor
│   │   ├── yandex/      # Yandex HTML parser & feature extractor
│   │   ├── baidu/       # Baidu HTML parser & feature extractor
│   │   ├── ecosia/      # Ecosia HTML parser & feature extractor
│   │   ├── hackernews.rs# Hacker News (Algolia API)
│   │   ├── github.rs    # GitHub Repositories Search API
│   │   ├── crates.rs    # Crates.io API
│   │   └── wikipedia.rs # Wikipedia OpenSearch API
│   ├── extract/         # Web page extraction, readability, OpenGraph & JSON-LD
│   ├── flareprox/       # FlareProx Cloudflare Worker proxy & Edge SERP generator
│   │   ├── client.rs    # Cloudflare REST v4 API client & regional placement
│   │   ├── worker_script.rs # Regional proxy worker script generator
│   │   └── edge_worker_script.rs # Main Edge SERP Worker with RRF & recycling
│   ├── jobs/            # Async batch rank-tracking queue & webhooks
│   ├── mcp/             # Model Context Protocol (stdio JSON-RPC) server & installer
│   ├── mega/            # Reciprocal Rank Fusion (RRF) multi-engine searcher
│   ├── rank/            # Smart neighbor probing & domain matching
│   ├── server/          # Axum HTTP routes, OpenAPI/Swagger UI, and middleware
│   └── suggest/         # Autocomplete / OpenSearch suggestion client
└── tests/               # Integration tests with HTML fixtures
```

## Cloudflare Edge & Regional FlareProx Architecture

Frontlane SERP can run either as a local/containerized daemon or directly at the edge across Cloudflare's global network:

```text
[User / AI Agent]
       │
       ▼
[Main Edge SERP Worker] (frontlane-serp-edge)
 ├── /docs (Interactive Swagger UI)
 ├── /mega/search (Multi-Engine Search & Edge RRF Fusion)
 └── /api/proxies/recycle (On-Demand Cloudflare v4 Worker Management)
       │
       ├─────────────────────────┬─────────────────────────┐
       ▼                         ▼                         ▼
 [FlareProx Lane 1]        [FlareProx Lane 2]        [FlareProx Lane 3]
 (Smart Placement: FRA)    (Smart Placement: FRA)    (Smart Placement: FRA)
 (German Egress IP)        (German Egress IP)        (German Egress IP)
       │                         │                         │
       ▼                         ▼                         ▼
   [Google DE]               [Bing DE]                 [DuckDuckGo]
```

### 1. Smart Placement & Regional Egress
Cloudflare Workers execute serverless code at nearest edge points by default. For localized search engine results (e.g. Google DE requiring German egress), FlareProx injects `placement: { "mode": "smart", "region": "aws:eu-central-1" }` into the Cloudflare Worker upload metadata, binding egress to the target jurisdiction without requiring dedicated residential proxy subscriptions.

### 2. Autonomous Proxy Recycling
When search engines trigger CAPTCHA or rate-limiting gates, the Main Edge Worker's `/api/proxies/recycle` endpoint utilizes Cloudflare's REST API v4 to dynamically provision new localized worker scripts and delete stale proxy lanes on demand.

## Core Trait: `SearchEngine`

Every search engine implements `crate::core::engine::SearchEngine`:

```rust
#[async_trait]
pub trait SearchEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>>;
    async fn search_image(&self, query: &Query) -> Result<Vec<SearchResult>>;
}
```

## Mega Search & Reciprocal Rank Fusion (RRF)

When querying `/mega/search`, the `MegaSearcher` executes requests across the requested engines concurrently using Tokio `FuturesUnordered`.

Results are deduplicated and merged using **Reciprocal Rank Fusion (RRF)**:
$$RRF(d) = \sum_{e \in E} \frac{1}{k + r_e(d)}$$
where $k = 60$ and $r_e(d)$ is the rank of URL $d$ in engine $e$.

Outputs include:
- `score`: Rounded RRF relevance score.
- `engine_consensus`: The count of unique search engines returning the target URL.
- `engines`: Array of engine names confirming the result.

## Domain-Restricted Crawler (`/crawl`)

The `Crawler` operates an asynchronous breadth-first search (BFS):
1. **Robots.txt Enforcement**: Respects `User-agent`, `Disallow`, and `Allow` rules unless `respect_robots: false`.
2. **SSRF Guard**: Validates every target IP via `network_guard.rs` to block internal cloud metadata endpoints (`169.254.169.254`), loopbacks, and RFC 1918 subnets.
3. **Domain Filtering**: Confines links strictly to the origin host or explicit `allowed_domains`.
4. **Metadata Extraction**: Extracts titles, readable markdown, Schema.org JSON-LD scripts, and OpenGraph/Twitter card tags.

## MCP Server Layer

Frontlane SERP exposes a full Model Context Protocol (MCP) server over standard I/O (`frontlane-serp mcp`), allowing LLM agents (Claude Desktop, Cursor, Antigravity) to call tools:
- `serp_search`: Single-engine search.
- `mega_search`: Multi-engine RRF search.
- `check_rank`: Domain/subdomain ranking prober.
- `suggest_keywords`: Autocomplete expansion.
- `extract_content`: Single-page markdown & metadata extraction.
- `crawl_site`: Multi-page domain crawl.
