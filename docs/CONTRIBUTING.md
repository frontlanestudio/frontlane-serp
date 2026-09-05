# Contributing to Frontlane SERP

## Development Setup

### Prerequisites

- Rust 1.80+ (stable toolchain with `clippy` and `rustfmt`)
- Optional: `cargo-watch` for continuous testing

### Clone, Build, and Run

```bash
git clone https://github.com/frontlanestudio/frontlane-serp.git
cd frontlane-serp
cargo build
cargo run -- serve
```

### Verification Commands

Run these before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Adding a New Search Engine

1. Create engine implementation in `src/engines/<name>.rs` or `src/engines/<name>/mod.rs`.
2. Implement the `SearchEngine` trait from `src/core/engine.rs`.
3. Export the engine in `src/engines/mod.rs` and register it in `build_all_engines` in `src/main.rs`.
4. Add engine tests in `tests/<name>_test.rs` with mock response fixtures.
5. Update docs and `config.yaml` if engine requires special headers or rate limits.

## Pull Request Process

1. Describe what changed and why.
2. Ensure `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with 0 errors.
3. Ensure all unit and integration tests pass via `cargo test`.
4. Update `README.md` and `docs/openapi.yaml` when API routes or parameters change.
