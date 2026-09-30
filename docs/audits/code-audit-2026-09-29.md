# frontlane-serp code audit — 2026-09-29

Generated with rust-code-mcp (hypergraph snapshot `4fe8ff32…`, HEAD `93b14f8`), findings spot-checked against source.

## Index status

| Index | Status |
|---|---|
| Hypergraph (HIR) | Built: 3 crates, 125 modules, 1,048 items, 2,882 usage edges |
| BM25 + vector search | **Failed**: embedding model `model_optimized.onnx` could not be downloaded. `search`, `get_similar_code`, `similar_to_item` unavailable until fixed (check network or model cache, then re-run `index_codebase`). |

Workspace: ~24.5k lines in `src/`, 359 fns + 172 methods, 233 structs, 28 enums, 2 traits.

## Clean areas

- **unsafe:** 0 blocks.
- **Channels:** no mpsc/crossbeam/flume construction, so no unbounded channels.
- **Global state:** 10 `LazyLock<Regex>` + 1 `OnceLock<EnrichmentConfig>` (`core::domain`). No `static mut`.
- **transmute / unwrap_unchecked:** none.
- **SSRF guard:** `network_guard::validate_public_url` is used by the crawler, extractor, contacts, job queue and server handlers.

## Findings (ranked)

### 1. HIGH (FIXED): `mcp install` can wipe a user's MCP client config
`src/mcp/installer.rs`, `install_mcp`: if the existing config file doesn't parse as JSON (for example a trailing comma, JSONC comments, or a partial write), `serde_json::from_str(..).unwrap_or_else(|_| json!({}))` silently swaps in `{}`. The code then writes that back with `fs::write`, which **deletes every other MCP server and setting** in Claude Desktop, Cursor and similar clients. The same happens if the root isn't an object.
**Fix:** on parse failure, stop with an error, or back up the file to `*.bak` before rewriting. Write atomically (temp file + rename).
(The two `.as_object_mut().unwrap()` calls flagged by the MCP are guarded by the `is_object()` reset, so they are safe.)

### 2. MED: `frontlane-net` is a dependency but barely used, and its types are duplicated
- Across the whole workspace, `frontlane_serp` uses exactly **one** symbol from `frontlane_net`: `detector::detect_cloudflare` (`src/extract/mod.rs:21`). The other 52 pub items in `frontlane_net` have no consumer.
- `frontlane_serp` keeps its own copies of types that `frontlane_net` also defines: `ImpersonationPool`, `ImpersonatingClient`, `ProxyLaneKey`, `BrowserPlatform`, robots `Rule`, and `Result`.
- `Cargo.toml` enables `frontlane-net` with `features = ["impersonate"]` unconditionally. If that feature pulls in wreq/BoringSSL on the net side, every build pays for a cmake BoringSSL compile, even though frontlane-serp's own `impersonate` feature is meant to be opt-in. This needs checking in `../frontlane-net/Cargo.toml`, which the audit couldn't read.
**Decide:** either migrate `core::impersonate`, `core::proxy` and `crawl::robots` onto `frontlane_net` and delete the local copies, or drop the dependency and inline `detect_cloudflare`.

### 3. MED: dead or duplicate modules in `core`
- `core::resilient` (176 lines) has its own `CircuitBreaker` / `CircuitState` / `ResilientSearcher`. It is only re-exported from `core/mod.rs` and never used. The server uses `core::circuit_breaker::CircuitBreakerManager` instead. Delete it, or merge the two.
- `core::rate_limiter::RateLimiter` is re-exported but never constructed. It also has a latent panic: `RateLimiter::new(0, ..)` gives `refill_rate = 0`, then `wait_secs = inf`, and `Duration::from_secs_f64(inf)` panics. Delete it, or clamp `requests >= 1`.
- The same config shape is defined twice: `config::ProxyEntry` vs `core::proxy::ProxyEntry`, and `config::CircuitBreakerConfig` vs `core::circuit_breaker::CircuitBreakerConfig` (the second is aliased as `CbConfig` in `server/state.rs`).

### 4. LOW: `Selector::parse("…").unwrap()` rebuilt on every call
There are 11 in `audit::parser::parse_page_audit`, 2 in `extract::contacts::extract_visible_text`, and 1 in `engines::bing::parser::parse_html` (inside a per-result loop). They won't panic because the literals are valid, but each call re-parses the selector. Move them to `LazyLock<Selector>` statics, the same pattern the crate already uses for `Regex`.

### 5. LOW: recursion over untrusted JSON-LD
`audit::parser::extract_schema_types`, `audit::parser::extract_ratings_and_reviews` and `extract::contacts::traverse_json_ld_for_contacts` each call themselves (direct recursion). Depth is effectively capped by serde_json's default recursion limit of 128, so stack-overflow risk is low. Keep that limit in place, and don't enable `unbounded_depth`.

### 6. LOW: API hygiene
- **Visibility:** 636 `pub` declarations and **0** `pub(crate)`. dead_pub_report lists 348 `pub` items in `frontlane_serp` that the binary never touches. The biggest groups are `core::types` (23), `engines::google` (23), `engines::duckduckgo` (21), `engines::bing` (19), `engines::ecosia` (18) and `engines::yandex` (15). Narrow to `pub(crate)` whatever isn't meant as library API.
- **Docs:** 429 pure-`pub` items have no rustdoc.
- **Debug:** 33 `pub` structs lack `#[derive(Debug)]`. Examples: all 10 engine structs, `HttpClient`, `AppState`, `JobManager`, `Crawler`, `Extractor`, `MegaSearcher`, `ResponseCache`. Watch for fields that hold secrets (Cloudflare token in `CloudflareClient`); give those a manual `Debug` impl that redacts them.
- **Large files:** `src/main.rs` has 1,517 lines, 26 fns and cyclomatic complexity 240 (avg 9.2 per fn). Move command dispatch into `cli::*` handlers. `src/server/handlers.rs` has 1,710 lines and 43 fns; split it by route group.

### False positives / acceptable (no action)
- `core::rate_limiter::acquire` await-in-guard: the guard is explicitly `drop`ped before `.await`.
- `engines::baidu::url::build_url` `and_hms_opt(0,0,0).unwrap()` can't fail.
- `jobs::queue::submit_batch_rank` `sem.acquire().await.unwrap()`: the semaphore is never closed, and concurrency is clamped to 1..=10.
- `core::impersonate` `unreachable!()`: these are stubs for when the feature is disabled, and the constructor always errors first.
- circuit_breaker / proxy / impersonate "self_recursion": these are calls to same-named methods on another type, not real recursion.

## Suggested order
1. Fix the installer overwrite (#1). It's small and prevents user data loss.
2. Delete `core::resilient` and `core::rate_limiter`, and merge the duplicate config types (#3).
3. Decide what to do with `frontlane-net` (#2).
4. Do the hygiene pass: `LazyLock<Selector>`, `pub(crate)`, `Debug`, and add `#![warn(missing_docs)]` gradually.
