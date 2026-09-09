# Changelog

All notable changes to **Frontlane SERP** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-06

### Added
- **Pure Async Rust Rewrite**: High-performance asynchronous SERP engine powered by Tokio and Axum 0.8.
- **10 Search Engines**:
  - Web & image search: Google, Bing, DuckDuckGo, Yandex, Baidu, Ecosia.
  - Developer verticals: Hacker News (Algolia API), GitHub Repositories, Crates.io, and Wikipedia (OpenSearch).
- **Megasearch with RRF**: Reciprocal Rank Fusion aggregation across multiple search engines with engine consensus scoring.
- **Anti-Bot & Anti-Detection Pipeline**:
  - Fingerprint impersonation via optional `impersonate` feature (BoringSSL/JA3/HTTP2).
  - Pure-Rust default HTTP stack with Rustls TLS.
  - Cloudflare Turnstile managed challenge handling via 2Captcha/CapSolver integration.
  - Per-proxy-lane cookie and `cf_clearance` caching.
- **Domain-Restricted Web Crawler**: Async BFS web crawler with depth limits, domain filtering, and robots.txt parsing.
- **Content & Metadata Extractor**: Readability algorithm, OpenGraph tags, Twitter Card metadata, and Schema.org JSON-LD extraction.
- **Smart Rank Probing**: Optimized neighbor probing (`[P-1, P, P+1]`) for SERP position tracking.
- **Serp Competitor Audit Subsystem**: Comprehensive site audit analysis with topical outlines, gap summaries, review parsing, and opportunity scoring.
- **FlareProx Pure-Rust Engine**: Automated Cloudflare Worker proxy provisioning, key rotation, and health checking.
- **Model Context Protocol (MCP)**: Native stdio JSON-RPC server implementing the MCP specification for LLM agents.
- **Cloudflare Edge Deployment (`frontlane-serp edge`)**:
  - Full serverless deployment of Frontlane SERP to Cloudflare Workers with multi-engine search, Reciprocal Rank Fusion (RRF), and embedded Swagger UI.
  - Multi-lane FlareProx proxy coordination with regional placement and on-demand proxy lifecycle management (`/api/proxies/recycle`).
- **Regional FlareProx Smart Placement**:
  - Support for `--region` (e.g. `de`, `uk`, `us`, `jp`, `sg`) routing proxy worker execution through localized data centers (e.g. Frankfurt/FRA for Germany).
- **Interactive Swagger UI & OpenAPI Specification**:
  - Embedded Swagger UI served at `/docs` (with browser redirects from root `/`) and raw OpenAPI 3.0 YAML at `/openapi.yaml`.
- **Diagnostic Command (`frontlane-serp doctor`)**:
  - Self-diagnostic tool testing platform environment, TLS impersonation, port availability, live search engine access, and MCP client config status.
- **1-Click MCP Setup (`frontlane-serp mcp install`)**:
  - Automated configuration and verification for Claude Desktop and Cursor AI agents.
- **Drop-in Compatibility**: Emulation endpoints for Serper (`/v1/serper/search`) and SerpApi (`/v1/serpapi/search`).
- **Flexible Export Formats**: Full support for JSON, NDJSON, Markdown, plain text, and RFC 4180 CSV outputs.

### Security
- Comprehensive SSRF protection blocking loopback, RFC 1918, link-local, and cloud metadata IP ranges (`169.254.169.254`) on webhooks, proxies, and crawler requests.
- Enforced secure TLS certificate verification by default (`insecure: false`).
- FlareProx proxy gateway authentication via `X-Flareprox-Key` header and query key validation.

### Changed
- Concurrency hardening: Parallelized crawler BFS queue execution and batch extraction handlers.
- Memory safety: Replaced unbounded background job memory queues with bounded LRU/TTL caches.
