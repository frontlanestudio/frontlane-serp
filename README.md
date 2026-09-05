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
- **Smart Rank Probing**: Probes rankings using target neighbor page windows (`[P-1, P, P+1]`) to cut crawling volume by 70–90%.
- **Keyword Autocomplete**: Instant suggestion expansion across Google, Bing, DuckDuckGo, and Ecosia.
- **Native MCP Server**: Instant integration for Claude Desktop, Cursor, Antigravity, and Claude Code (`frontlane-serp mcp`).
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
# mode=any returns the first engine that responds
curl "http://127.0.0.1:7000/mega/search?engines=bing,google&text=golang+vs+rust&extract=1&mode=any"
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

## SDKs & Examples

Official client packages. Each works against your self-hosted server (set `baseUrl`) or the [hosted API](https://openserp.org/cloud) (set `apiKey`):

| Type                        | Package                                                                                      | Source                                                              | Install                         |
| --------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------- |
| JavaScript / TypeScript SDK | [`@openserp/sdk`](https://www.npmjs.com/package/@openserp/sdk)                               | [openserpapi/sdk-js](https://github.com/openserpapi/sdk-js)         | `npm install @openserp/sdk`     |
| Python SDK                  | [`openserp`](https://pypi.org/project/openserp/)                                             | [openserpapi/sdk-python](https://github.com/openserpapi/sdk-python) | `pip install openserp`          |
| MCP server (AI agents)      | [`@openserp/mcp`](https://www.npmjs.com/package/@openserp/mcp)                               | [openserpapi/mcp](https://github.com/openserpapi/mcp)               | `npx @openserp/mcp`             |
| n8n community node          | [`@openserp/n8n-nodes-openserp`](https://www.npmjs.com/package/@openserp/n8n-nodes-openserp) | [openserpapi/n8n](https://github.com/openserpapi/n8n)               | Install via n8n community nodes |

See [**examples**](./examples) for small JavaScript and Python use cases covering search, AI grounding, SEO, content extraction, and image search.

```js
import { OpenSERP } from "@openserp/sdk";

// Use your self-hosted server
const client = new OpenSERP({ baseUrl: "http://localhost:7000" });
const { results } = await client.search({ engine: "google", text: "openserp", limit: 5 });
```

## Search Endpoints

Available engine names: `google`, `yandex`, `baidu`, `bing`, `duckduckgo`, `ecosia`.

Dedicated engine endpoints:

```bash
curl "http://127.0.0.1:7000/google/search?text=golang&limit=10"
```

Image search:

```bash
curl "http://127.0.0.1:7000/bing/image?text=golang+logo&limit=10"
```

Megasearch:

```bash
curl "http://127.0.0.1:7000/mega/search?text=golang&limit=10"
```

`/mega/search` returns the same envelope as engine endpoints plus `clusters`: results are deduplicated by normalized URL, and clusters keep the per-engine occurrences and ranks.

| Mode       | Best for                             | Behavior                                       |
| ---------- | ------------------------------------ | ---------------------------------------------- |
| `balanced` | Most multi-engine SERP workflows     | Queries engines in parallel and merges results |
| `fast`     | Lowest latency                       | Uses the fastest available engine              |
| `any`      | Fallback-style availability checking | Tries engines sequentially until one responds  |

<details>
<summary>More megasearch examples</summary>

```bash
# Fast mode
curl "http://127.0.0.1:7000/mega/search?text=golang&mode=fast&engines=google,bing,yandex"

# Any mode
curl "http://127.0.0.1:7000/mega/search?text=golang&mode=any&engines=google,yandex,bing"

# Balanced mode with aggregation controls
curl "http://127.0.0.1:7000/mega/search?text=golang&mode=balanced&dedupe=true&merge=true"

# Advanced filtering
curl "http://127.0.0.1:7000/mega/search?text=golang&engines=google,bing&limit=20&date=20250101..20251231&lang=EN&region=US"

# Image megasearch
curl "http://127.0.0.1:7000/mega/image?text=golang+logo&limit=20"
```

</details>

List engines:

```bash
curl "http://127.0.0.1:7000/mega/engines"
```

URL extraction:

```bash
# Extract one URL as JSON
curl "http://127.0.0.1:7000/extract?url=https://example.com&mode=auto"

# Return clean page markdown
curl "http://127.0.0.1:7000/extract?url=https://example.com&format=markdown"

# Extract several URLs at once - returns a bare [{page_content, metadata}] array
# (Open WebUI external loader compatible); failed URLs become items with metadata.error
curl -X POST "http://127.0.0.1:7000/extract/batch" \
  -H "Content-Type: application/json" \
  -d '{"urls":["https://example.com","https://go.dev"],"mode":"fast"}'

# Embed extracted content under the top search results
curl "http://127.0.0.1:7000/google/search?text=llm+observability&extract=2&format=markdown"
```

## CLI Search

No server required - query an engine straight from the terminal. The CLI shares the same engines, formats, and filters as the API.

```sh
openserp search ecosia "weather in london" --format markdown
```

<details>
<summary>CLI output and more examples</summary>

```markdown
# Search results for "weather in london"

**Query:** weather in london - **Engines:** ecosia - **Took:** 866ms

## Results

### 1. London - BBC Weather

**bbc.com › weather › 2643743** - organic

Latest forecast for London ... Tonight will continue dry, and there will be mainly clear skies. Just a few patches of cloud drifting in from the north at times.

-> https://www.bbc.com/weather/2643743

### 2. London (Greater London) weather - Met Office

**weather.metoffice.gov.uk › forecast › gcpvj0v07** - organic

Remaining warm with light winds and dry. Possibly cloudy at times Monday and Tuesday, then Wednesday sunnier conditions are likely.

-> https://weather.metoffice.gov.uk/forecast/gcpvj0v07

### 3. London, London, United Kingdom Weather Forecast

**accuweather.com › en › gb › london › ec4a-2 › wea…** - organic

London, London, United Kingdom Weather Forecast, with current conditions, wind, air quality, and what to expect for the next 3 days.

-> https://www.accuweather.com/en/gb/london/ec4a-2/weather-forecast/328328
```

More CLI examples:

```sh
# JSON is the default format
frontlane-serp search google "rust async web" --limit 20

# Developer engines: Hacker News & GitHub
frontlane-serp search hn "show hn AI" --limit 10
frontlane-serp search gh "serp scraper" --limit 10
frontlane-serp search crates "tokio" --limit 5
frontlane-serp search wiki "Rust (programming language)" --limit 5

# Plain text, German results
frontlane-serp search yandex "wetter berlin" --format text --lang DE --region DE

# Restrict to a site and stream NdJSON
frontlane-serp search bing "release notes" --site github.com --format ndjson

# Embed clean page content & metadata from the top 2 results
frontlane-serp search google "llm observability" --extract 2 --format markdown

# Rank checking with smart neighbor probing
frontlane-serp rank github.com "rust serp api" --engine google --strategy smart

# Keyword suggestions
frontlane-serp suggest "async rust" --engine google

# Domain-bounded website crawl
frontlane-serp crawl https://example.com --max-depth 2 --max-pages 10

# Launch MCP server for Claude Desktop / Cursor / Antigravity
frontlane-serp mcp
```

</details>

Run `frontlane-serp search --help` for the full flag list. Engine names: `google`, `bing`, `duckduckgo` (`ddg`), `yandex`, `baidu`, `ecosia`, `hackernews` (`hn`), `github` (`gh`), `crates`, `wikipedia` (`wiki`).

## Query Parameters

Common parameters:

| Parameter      | Description                                                                                                                                                                                             | Example                              |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `text`         | Search query                                                                                                                                                                                            | `rust programming`                   |
| `lang`         | Language code                                                                                                                                                                                           | `EN`, `DE`, `RU`, `ES`               |
| `region`       | Market/location hint. Countries/locales work across engines; Google also accepts city names via `uule`; Yandex accepts numeric `lr`.                                                                    | `DE`, `en-GB`, `Berlin`, `213`       |
| `date`         | Date range                                                                                                                                                                                              | `20250101..20251231`                 |
| `file`         | File extension                                                                                                                                                                                          | `pdf`, `doc`, `xls`                  |
| `site`         | Site-specific search                                                                                                                                                                                    | `github.com`                         |
| `limit`        | Number of organic results, max 100.                                                                                                                                                                     | `25`, `50`                           |
| `start`        | Pagination offset                                                                                                                                                                                       | `0`, `10`, `20`                      |
| `format`       | Output format                                                                                                                                                                                           | `json`, `markdown`, `text`, `ndjson` |
| `extract`      | Fetch and embed target-page content & metadata for top web results. Depth: `0`/`false` off, `true`/`1` top result, `N` top N (1-5).                                                                    | `1`, `3`, `true`                     |
| `extract_mode` | Extraction strategy: raw HTTP first, raw only, or browser-rendered                                                                                                                                      | `auto`, `fast`, `rendered`           |

## Proxy Support

Frontlane SERP supports HTTP, HTTPS, and SOCKS5 proxies.

Simple global proxy:

```bash
frontlane-serp serve --proxy socks5://127.0.0.1:1080
frontlane-serp search bing "query" --proxy http://user:pass@127.0.0.1:8080
```

Advanced proxy configuration is available in [config.yaml](./config.yaml). You can enable tagged proxy pools and per-request override via `X-Use-Proxy: <tag>` or `X-Use-Proxy: direct`.

## API Docs

Once the server is running, the interactive docs are available locally:

- Swagger UI: `http://127.0.0.1:7000/docs`
- OpenAPI YAML: `http://127.0.0.1:7000/openapi.yaml`

To browse the spec without running the server, see [docs/openapi.yaml](./docs/openapi.yaml). For architecture details, see [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE).

## Contributing

Contributions are welcome. See [docs/CONTRIBUTING.md](./docs/CONTRIBUTING.md).

## Feedback & Community

- [GitHub Issues](https://github.com/frontlanestudio/frontlane-serp/issues) - bugs, feature ideas, and reproducible issues.
