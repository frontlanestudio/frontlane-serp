# Good First Issue Backlog

Welcome! If you're looking to contribute to **Frontlane SERP**, these issues and roadmap ideas are great places to start.

Before getting started:
- Read [`CONTRIBUTING.md`](CONTRIBUTING.md).
- Adding a new search engine? Check [`ADDING_AN_ENGINE.md`](ADDING_AN_ENGINE.md).

---

## Suggested Starter Tasks

### 1. Brave Search Engine
- **Area**: New engine (`src/engines/brave/` or `src/engines/brave.rs`).
- **Goal**: Add initial HTML parsing and mock fixture tests for Brave Search results.
- **Guide**: Follow [`ADDING_AN_ENGINE.md`](ADDING_AN_ENGINE.md).

### 2. Live Smoke-Check Script
- **Area**: Release tooling (`scripts/smoke-check.sh`).
- **Goal**: Write an automated test script that builds the release binary, starts `frontlane-serp serve`, polls `/health` until ready, issues test queries to `localhost:7000`, and cleanly shuts down.

### 3. OpenSearch Engine Plugin Definition
- **Area**: Extensibility (`src/engines/`).
- **Goal**: Support declarative engines via OpenSearch XML description autodiscovery.

### 4. Result Cache Enhancements
- **Area**: Core cache (`src/core/cache.rs`).
- **Goal**: Add cache hit/miss statistics and configurable Redis backend support alongside Moka memory cache.

---

## Getting Help

Feel free to open an issue or pull request with a draft/WIP prefix if you would like feedback on your implementation!
