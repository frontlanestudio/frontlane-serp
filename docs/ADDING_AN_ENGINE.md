# Adding a Search Engine

This guide describes how to add a new search engine to **Frontlane SERP**.

Keep initial pull requests small and focused: start with web search, deterministic HTML parsing tests using mock fixtures, and engine registration. Add image search or advanced parameters in follow-up pull requests.

---

## 1. Engine Directory Structure

Engines live in `src/engines/`. Standard engines with dedicated HTML parsing and feature extraction live in their own subdirectory:

```text
src/engines/<name>/
├── mod.rs        # Engine struct and SearchEngine trait implementation
├── parser.rs     # HTML parsing and result extraction (using `scraper`)
└── features.rs   # Optional SERP feature extractors (knowledge panels, PAA, etc.)
```

Simpler API-based engines can be implemented as a single file, e.g., `src/engines/<name>.rs`.

---

## 2. Implement the `SearchEngine` Trait

Every engine implements the async trait `crate::core::engine::SearchEngine`:

```rust
use async_trait::async_trait;
use crate::core::engine::SearchEngine;
use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::types::{Envelope, ImageEnvelope, Query, SearchResult};

pub struct MyEngine {
    client: HttpClient,
    rate_limiter: Arc<RateLimiter>,
}

#[async_trait]
impl SearchEngine for MyEngine {
    fn name(&self) -> &str {
        "myengine"
    }

    async fn search(&self, query: &Query) -> Result<Vec<SearchResult>> {
        // Build URL, fetch page/API via self.client, parse results
        todo!()
    }

    async fn search_image(&self, query: &Query) -> Result<Vec<SearchResult>> {
        // Optional image search support
        todo!()
    }
}
```

### Best Practices for Parsers
- Prefer stable data attributes (`data-testid`, semantic `<article>`, `role="..."`) over minified or dynamically generated CSS class names.
- When selectors are brittle across regions, provide 2 or 3 fallback CSS selectors.
- Always normalize URLs (convert protocol-relative `//` and query tracking redirects).

---

## 3. Register the Engine

1. **Export the Engine**: Add `pub mod <name>;` to [src/engines/mod.rs](file:///Users/bhubbard/PROJECTS/frontlane-serp/src/engines/mod.rs).
2. **Register in the Engine Registry**: Add the engine instance to `build_all_engines` in [src/main.rs](file:///Users/bhubbard/PROJECTS/frontlane-serp/src/main.rs).
3. **Configure Rate Limits**: Add default concurrency and rate limit settings in [config.yaml](file:///Users/bhubbard/PROJECTS/frontlane-serp/config.yaml).
4. **Documentation**: Update [README.md](file:///Users/bhubbard/PROJECTS/frontlane-serp/README.md) and [docs/openapi.yaml](file:///Users/bhubbard/PROJECTS/frontlane-serp/docs/openapi.yaml).

---

## 4. Add Tests

Add unit and integration tests in `tests/<name>_test.rs`:
- Use sanitized, static HTML fixtures in `tests/fixtures/<name>/` to verify title, URL, snippet, and position extraction deterministically.
- All default tests must run offline and pass without network or browser dependencies:

```bash
cargo test --test <name>_test
```

---

## 5. Verification Before Submitting PR

Before opening your pull request, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
