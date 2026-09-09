![Frontlane SERP](./logo.svg)

# Frontlane SERP

[![CI](https://github.com/frontlanestudio/frontlane-serp/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/frontlanestudio/frontlane-serp/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/frontlanestudio/frontlane-serp)](https://github.com/frontlanestudio/frontlane-serp/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Frontlane SERP** is a fast, open-source SERP API, CLI, and Model Context Protocol (MCP) server written in pure async Rust for Google, Bing, DuckDuckGo, Yandex, Baidu, Ecosia, Hacker News, GitHub, Crates.io, and Wikipedia.

No API keys, no per-search billing: one command gives you live, structured search results on localhost - including engines the paid APIs don't cover. Use it as a search and retrieval tool for LLMs and AI agents, or as a backend for SEO rank tracking and site crawling.

![Frontlane SERP CLI demo](./docs/demo.gif)

## Features

- **10 Search Engines**: Google, Bing, DuckDuckGo, Yandex, Baidu, Ecosia, Hacker News, GitHub, Crates.io, and Wikipedia.
- **Megasearch with RRF**: One query across several engines at once with **Reciprocal Rank Fusion (RRF)** and engine consensus scoring.
- **Deep Content & Metadata Extraction**: Search results plus clean markdown, OpenGraph tags, Twitter cards, and Schema.org JSON-LD structured data.
- **Domain-Restricted Web Crawler**: Async BFS crawler with depth limits, domain filtering, robots.txt compliance, and SSRF guard protection.
- **Cloudflare & Anti-Bot Hardening**: Managed-challenge (Turnstile) solving via 2Captcha/CapSolver with per-proxy-lane `cf_clearance` caching, reCAPTCHA v2/v3 and hCaptcha detection/solving for non-Cloudflare gates, and browser TLS/JA3 fingerprint impersonation so extraction and crawling requests match real Chrome at the handshake level, not just in headers.
- **Smart Rank Probing**: Probes rankings using target neighbor page windows (`[P-1, P, P+1]`) to cut crawling volume by 70–90%.
- **Keyword Autocomplete**: Instant suggestion expansion across Google, Bing, DuckDuckGo, and Ecosia.
- **Cloudflare Edge Deployment & FlareProx**: Deploy the entire SERP search engine, RRF fusion, and proxy rotation to Cloudflare Workers with one command (`frontlane-serp edge deploy --region de`). Features regional proxy placement and dynamic on-demand proxy worker creation/recycling via the Cloudflare REST API.
- **Diagnostic Tool & 1-Click MCP Setup**: Built-in `frontlane-serp doctor` for self-diagnosing network, ports, TLS, and engine access, plus `frontlane-serp mcp install` for instant Claude Desktop and Cursor setup.
- **Interactive Swagger UI & OpenAPI**: Full OpenAPI 3.0 specification served directly with Swagger UI at `/docs` (and root browser redirect).
- **Drop-In Serper & SerpApi Compatibility**: Drop-in emulation for `POST /v1/serper/search` and `GET /v1/serpapi/search`.
- **Embedded Batch Jobs**: Asynchronous rank-tracking queue with bounded concurrency and webhook notifications.

## Quick Start

### Build From Source (Rust)

Requires Rust 1.80+:

```sh
git clone https://github.com/frontlanestudio/frontlane-serp.git
cd frontlane-serp
cargo build --release
./target/release/frontlane-serp serve
```

### Install via Cargo

```sh
cargo install --git https://github.com/frontlanestudio/frontlane-serp.git
frontlane-serp search duckduckgo "rust async web" --format markdown
```

### Docker

```sh
# Build and run locally
docker build -t frontlane-serp .
docker run --rm -p 127.0.0.1:7000:7000 frontlane-serp:latest serve

# Or with docker compose
docker compose up
```

### First request

```sh
curl "http://127.0.0.1:7000/mega/search?engines=bing,duckduckgo&text=rust+async&extract=1"
```

<details>
<summary>Example JSON response</summary>

```json
{
  "query": {
    "text": "golang vs rust",
    "engines_requested": ["bing", "google"]
  },
  "meta": {
    "request_id": "019ecdc0-a66d-79a4-9d2b-9e9b480d495e",
    "requested_at": "2026-06-16T00:06:55Z",
    "took_ms": 720,
    "engines_responded": ["bing"],
    "engines_failed": [],
    "version": "2.1"
  },
  "results": [
    {
      "id": "s_5a8273f16b19ab64",
      "rank": 1,
      "type": "organic",
      "title": "The Go Programming Language",
      "url": "https://go.dev/",
      "display_url": "go.dev",
      "snippet": "Get Started Playground Tour Stack Overflow Help Packages Standard Library …",
      "domain": "go.dev",
      "favicon": "https://go.dev/favicon.ico",
      "position": {
        "absolute": 1
      },
      "engine": "bing",
      "domain_info": {
        "tld": "dev",
        "sld": "go",
        "category": ""
      },
      "extracted": {
        "title": "Build simple, secure, scalable systems with Go",
        "format": "markdown",
        "content": "## Build simple, secure, scalable systems with Go\n\n![Go Gopher climbing a ladder.](https://go.dev/images/gophers/ladder.svg)\n\n- “At the time, no single team member knew Go, but **within a month, everyone was writing in Go** and we were building out the endpoints. ........",
        "mode_used": "fast",
        "fetched_at": "2026-06-16T00:06:56Z"
      }
    },
    {
      "id": "s_1a364ebcb3035539",
      "rank": 2,
      "type": "organic",
      "title": "Go (programming language) - Wikipedia",
      "url": "https://en.wikipedia.org/wiki/Go_(programming_language)",
      "display_url": "en.wikipedia.org › wiki › Go_(programming_language)",
      "snippet": "In Go's package system, each package has a path (e.g., \"compress/bzip2\" or \"golang.org/x/net/html\") and a name (e.g., bzip2 or html). …",
      "domain": "en.wikipedia.org",
      "favicon": "https://en.wikipedia.org/favicon.ico",
      "position": {
        "absolute": 2
      },
      "engine": "bing",
      "domain_info": {
        "tld": "org",
        "sld": "wikipedia",
        "category": ""
      },
      "classification": {
        "content_type": "article",
        "source_hint": "encyclopedia"
      }
    },
    ...
  ],
  "serp_features": [],
  "pagination": {
    "page": 1,
    "has_more": false,
    "next_start": 10
  },
  "clusters": [
    {
      "id": "c_f20b23a020101dce",
      "canonical_url": "https://go.dev/",
      "domain": "go.dev",
      "title": "The Go Programming Language",
      "occurrences": [
        {
          "engine": "bing",
          "rank": 1,
          "result_id": "s_5a8273f16b19ab64"
        }
      ],
      "engines_count": 1,
      "best_rank": 1,
      "score": 0.5
    },
    ...
  ]
}
```

</details>

## Endpoints & Capabilities

### 1. Dedicated Search & Image Endpoints

Directly query any of the 10 supported engines (`google`, `bing`, `duckduckgo`, `yandex`, `baidu`, `ecosia`, `hackernews`, `github`, `crates`, `wikipedia`):

```bash
# Web search (Google)
curl "http://127.0.0.1:7000/google/search?text=rust+axum&limit=10"

# Developer engine: Crates.io
curl "http://127.0.0.1:7000/crates/search?text=tokio&limit=5"

# Developer engine: Hacker News
curl "http://127.0.0.1:7000/hackernews/search?text=show+hn&limit=5"

# Developer engine: GitHub
curl "http://127.0.0.1:7000/github/search?text=rust+serp&limit=5"

# Image search
curl "http://127.0.0.1:7000/bing/image?text=rust+mascot&limit=10"
```

### 2. Megasearch with Reciprocal Rank Fusion (RRF)

`/mega/search` executes across multiple search engines concurrently in parallel and combines results using industry-standard **Reciprocal Rank Fusion (RRF, $k=60$)**, consensus tracking, and cross-engine clustering:

```bash
# Query Google, Bing, DuckDuckGo, and Hacker News simultaneously
curl "http://127.0.0.1:7000/mega/search?engines=google,bing,duckduckgo,hackernews&text=rust+vs+go&limit=10"
```

Response includes:
- `score`: RRF fused ranking score.
- `engine_consensus`: Number of engines confirming the result.
- `engines`: Array of engines where the URL appeared.
- `clusters`: Aggregated domain & URL occurrence statistics.

### 3. Domain-Bounded Website Crawler (`/crawl`)

Asynchronously crawl websites within strict domain boundaries with built-in SSRF protection:

```bash
curl -X POST "http://127.0.0.1:7000/crawl" \
  -H "Content-Type: application/json" \
  -d '{
    "start_url": "https://example.com",
    "max_depth": 2,
    "max_pages": 10,
    "respect_robots": true,
    "extract_markdown": true
  }'
```

Returns:
- Page status and title
- Readability-filtered clean Markdown content
- Discovered internal links
- Extracted OpenGraph and Schema.org JSON-LD structured data

### 4. Contact & Address Extraction (`/extract/contacts`)

Discover and aggregate all phone numbers, physical postal addresses, email addresses, and social profile links from a web page or entire site:

```bash
# Scan single page
curl "http://127.0.0.1:7000/extract/contacts?url=https://example.com"

# Crawl site for contact info
curl -X POST "http://127.0.0.1:7000/extract/contacts" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://example.com",
    "crawl": true,
    "max_pages": 10
  }'
```

### 5. Smart Rank Probing (`/{engine}/rank`)

Tracks rankings for target domains or URLs with intelligent neighbor page windowing (`[P-1, P, P+1]`) to eliminate 70–90% of crawling overhead:

```bash
# Check where example.com ranks for "rust programming" on Google
curl "http://127.0.0.1:7000/google/rank?target=example.com&keyword=rust+programming&strategy=smart"
```

### 5. Keyword Autocomplete & Suggestion (`/{engine}/suggest`)

Instant search suggestions powered by OpenSearch APIs:

```bash
curl "http://127.0.0.1:7000/google/suggest?q=rust+async"
# ["rust async", "rust async await", "rust async fn in trait", "rust async book"]
```

### 6. Drop-In 3rd-Party Compatibility (Serper & SerpApi)

Zero-code replacement for existing SEO tools, scripts, and libraries:
- **Serper API**: `POST http://127.0.0.1:7000/v1/serper/search`
- **SerpApi**: `GET http://127.0.0.1:7000/v1/serpapi/search?q=...`

Easily configure **SerpBear**, **OpenSEO**, **s33k**, or **LangChain** by simply pointing the API host to `http://localhost:7000`.

### 7. Asynchronous Batch Rank Queue & Webhooks

Queue large sets of keyword checks with bounded concurrency and outbound webhooks:

```bash
curl -X POST "http://127.0.0.1:7000/v1/rank/batch" \
  -H "Content-Type: application/json" \
  -d '{
    "webhook_url": "https://webhook.site/test",
    "queries": [
      {"engine": "google", "keyword": "rust api", "target": "github.com"},
      {"engine": "bing", "keyword": "serp scraper", "target": "frontlanestudio"}
    ]
  }'
```

---

## Cloudflare & Anti-Bot Handling

Page extraction and crawling (not search-engine scraping, which isn't Cloudflare-fronted) go through a few layers of hardening against managed challenges:

- **Managed-challenge (Turnstile) solving** via 2Captcha or CapSolver (`captcha.provider` / `captcha.apikey` in `config.yaml`), with the resulting `cf_clearance` cookie cached per proxy lane so it's reused across requests instead of re-solved every time.
- **reCAPTCHA v2/v3 and hCaptcha solving** for pages gated by one of these directly (not through Cloudflare) — the same 2Captcha/CapSolver credentials are reused. Unlike Turnstile there's no reusable clearance cookie: each is site-specific, so the extract/crawl response instead surfaces `captcha_challenge` (`"recaptcha_v2"` / `"recaptcha_v3"` / `"hcaptcha"`), `captcha_site_key`, and, once solved, `captcha_token` for the caller to submit to whatever endpoint that page actually expects.
- **Browser TLS/HTTP2 fingerprint (JA3/JA4) impersonation** for the actual outbound request. Cloudflare fingerprints the TLS ClientHello and HTTP/2 SETTINGS frame before it ever inspects headers, so spoofed `User-Agent`/`sec-ch-ua` headers alone don't help if the handshake underneath still looks like `rustls`. Extraction and crawling requests are instead sent through [`wreq`](https://github.com/0x676e67/wreq), a BoringSSL-backed fork of `reqwest`, emulating a real browser release's TLS + HTTP/2 fingerprint as one coherent bundle. Rather than one fixed profile for every request (itself a fingerprint at volume — real traffic isn't one constant JA3 hash), a small pool of recent Chrome/Firefox/Edge/Safari desktop profiles is used, picked deterministically per proxy lane so a lane's fingerprint stays stable for as long as its cached `cf_clearance` might still be valid, while different lanes tend to land on different profiles. Controlled by `app.browser_impersonation` in `config.yaml` (default `true`); if the impersonating client can't be built, or a request through it fails, it automatically falls back to the plain HTTP client and logs a warning rather than failing the request. Not available on Windows builds (see `src/core/impersonate.rs` for why) — Windows builds transparently use the plain client.
- Run `RUST_LOG=warn cargo run --example verify_impersonation` to confirm on your own network that impersonated requests are actually going out over BoringSSL rather than silently falling back.

---

## Model Context Protocol (MCP) Integration

Frontlane SERP includes a native Model Context Protocol (MCP) server over standard I/O for **Claude Desktop**, **Cursor**, **Antigravity**, and **Claude Code**.

Tools exposed to AI agents:
- `serp_search`: Single-engine search (Google, Bing, DDG, Yandex, Baidu, Ecosia, HN, GitHub, Crates, Wiki).
- `mega_search`: Multi-engine reciprocal rank fusion search.
- `crawl_site`: Multi-page domain crawl with markdown & metadata extraction.
- `extract_content`: Single-page content and metadata extraction.
- `check_rank`: Domain/subdomain ranking checker.
- `suggest_keywords`: Autocomplete suggestion generator.

### 1-Click Automated Setup (Recommended)

Automatically detect and configure Claude Desktop and Cursor with one command:

```sh
frontlane-serp mcp install
```

Check configuration status across installed AI clients:

```sh
frontlane-serp mcp status
```

To remove Frontlane SERP from your AI clients:

```sh
frontlane-serp mcp uninstall
```

### Manual Configuration

#### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "frontlane-serp": {
      "command": "/path/to/frontlane-serp",
      "args": ["mcp"]
    }
  }
}
```

#### Cursor

Add to `.cursor/mcp.json` or Cursor Settings > MCP:

```json
{
  "mcpServers": {
    "frontlane-serp": {
      "command": "/path/to/frontlane-serp",
      "args": ["mcp"]
    }
  }
}
```

---

## CLI Usage

Query directly from your terminal without starting a server:

```sh
# Run environment diagnostics (TLS impersonation, proxies, engine connectivity, MCP)
frontlane-serp doctor

# 1-Click AI Agent MCP setup
frontlane-serp mcp install

# Basic search
frontlane-serp search google "rust async web" --limit 10

# Developer engines
frontlane-serp search hn "show hn AI" --limit 5
frontlane-serp search gh "serp scraper" --limit 5
frontlane-serp search crates "tokio" --limit 5
frontlane-serp search wiki "Rust (programming language)" --limit 5

# Stream as NdJSON or Markdown
frontlane-serp search bing "release notes" --site github.com --format ndjson

# Extract markdown content from top 2 search results
frontlane-serp search google "llm observability" --extract 2 --format markdown

# Smart rank checking
frontlane-serp rank github.com "rust serp api" --engine google --strategy smart

# Automated batch rank tracking from file (CSV, JSON, or TXT)
frontlane-serp batch-rank --target calljacob.com --file calljacob_keywords.csv --engine duckduckgo --output results.json

# Keyword suggestions
frontlane-serp suggest "async rust" --engine google

# Extract contact info (phone numbers, physical addresses, emails, social profiles)
frontlane-serp contacts https://example.com

# Crawl site for contact info across about, contact, location, and team pages
frontlane-serp contacts https://example.com --crawl --max-pages 10 -F markdown -o contacts.md

# Launch MCP server
frontlane-serp mcp
```

---

## Query Parameters Reference

| Parameter      | Description                                                                                             | Example                              |
| -------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `text`         | Search query text                                                                                       | `rust programming`                   |
| `lang`         | Language code                                                                                           | `EN`, `DE`, `RU`, `ES`               |
| `region`       | Market/location hint (countries, locales, city names via Google `uule`, or Yandex `lr`)                 | `DE`, `en-GB`, `Berlin`, `213`       |
| `date`         | Date range filter                                                                                       | `20250101..20251231`                 |
| `file`         | File extension filter                                                                                   | `pdf`, `doc`, `xls`                  |
| `site`         | Restrict results to a specific domain                                                                   | `github.com`                         |
| `limit`        | Number of organic results to return (max 100)                                                           | `25`, `50`                           |
| `start`        | Pagination offset                                                                                       | `0`, `10`, `20`                      |
| `format`       | Output format: `json`, `markdown`, `text`, `ndjson`                                                     | `json`, `markdown`                   |
| `extract`      | Fetch and embed target-page markdown & metadata for top results (depth 1-5 or boolean)                  | `1`, `3`, `true`                     |
| `extract_mode` | Extraction strategy: `auto`, `fast` (raw HTTP), or `rendered`                                          | `auto`, `fast`                       |

---

## Proxy Support

Frontlane SERP supports HTTP, HTTPS, and SOCKS5 proxies.

```bash
# Global proxy via CLI
frontlane-serp serve --proxy socks5://127.0.0.1:1080
frontlane-serp search bing "query" --proxy http://user:pass@127.0.0.1:8080
```

Advanced tagged proxy pools and per-request overrides are available in [`config.yaml`](./config.yaml) via headers:
- `X-Use-Proxy: <tag>`
- `X-Use-Proxy: direct`

### FlareProx: Free Rotating Proxies via Cloudflare Workers

Frontlane SERP includes a native, pure-Rust implementation of [FlareProx](https://github.com/MrTurvey/flareprox), allowing you to automate the deployment of HTTP proxy endpoints on Cloudflare Workers. This grants you free IP rotation across Cloudflare's global edge network (up to 100,000 requests/day free).

```bash
# 1. Export Cloudflare credentials (or configure in config.yaml)
export CLOUDFLARE_API_TOKEN="your_api_token"
export CLOUDFLARE_ACCOUNT_ID="your_account_id"

# 2. Deploy 3 worker proxy endpoints (optionally in a specific region, e.g. Germany)
frontlane-serp flareprox create --count 3 --region de

# 3. List deployed endpoints
frontlane-serp flareprox list

# 4. Test connectivity and view observed egress IPs
frontlane-serp flareprox test

# 5. Sync active workers directly into config.yaml proxy pool
frontlane-serp flareprox sync

# 6. Delete specific workers or bulk cleanup
frontlane-serp flareprox delete flareprox-123456-abcdef
frontlane-serp flareprox cleanup
```

Once deployed, you can use any FlareProx worker endpoint directly with `--proxy` or through the proxy pool:

```bash
# Use FlareProx worker as proxy for search queries
frontlane-serp search google "rust async web" --proxy https://flareprox-worker.account.workers.dev
```

### Cloudflare Edge Deployment (`frontlane-serp edge`)

You can run the entire Frontlane SERP search, RRF fusion, and proxy rotation directly at Cloudflare's Edge, without hosting any local servers.

With a single command, `frontlane-serp edge deploy` will:
1. Deploy regional FlareProx worker proxies in the specified region (e.g. `--region de`, `--region uk`, `--region us`, `--region jp`).
2. Deploy the Main Edge SERP Worker running multi-engine search, consensus RRF fusion, and interactive Swagger UI at `/docs`.
3. Configure the Main Worker with `CF_API_TOKEN` so it can **dynamically create and destroy proxy workers on demand** when rate limits or CAPTCHAs occur.

```bash
# Deploy full Edge stack: Main SERP Worker + 3 German proxy workers
frontlane-serp edge deploy --name frontlane-serp-edge --proxies 3 --region de

# Inspect live edge status and proxy pool
frontlane-serp edge status --name frontlane-serp-edge

# Tear down the edge deployment and all associated FlareProx workers
frontlane-serp edge destroy --name frontlane-serp-edge
```

Once deployed, query your edge worker directly:
```bash
# Multi-engine search directly on Cloudflare Edge with RRF fusion
curl "https://frontlane-serp-edge.<account>.workers.dev/mega/search?engines=bing,duckduckgo&text=rust+async"

# Trigger an on-demand proxy recycling cycle via the edge worker
curl -X POST "https://frontlane-serp-edge.<account>.workers.dev/api/proxies/recycle?count=2&region=de" \
  -H "Authorization: Bearer <CLOUDFLARE_API_TOKEN>"
```

---

## API Documentation

Interactive documentation is served locally once the API server is running:
- **Swagger UI**: `http://127.0.0.1:7000/docs`
- **OpenAPI 3.0 Spec**: `http://127.0.0.1:7000/openapi.yaml` (also in [`docs/openapi.yaml`](./docs/openapi.yaml))
- **Architecture Details**: See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)

---

## Contributing

Contributions are welcome! Please read [`docs/CONTRIBUTING.md`](./docs/CONTRIBUTING.md) for local development setup and guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
</content>
