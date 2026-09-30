CARGO ?= cargo
BINARY ?= frontlane-serp

.PHONY: all build release test test-integration lint check run fmt clean

all: build

build:
	$(CARGO) build

release:
	$(CARGO) build --release

test:
	$(CARGO) test

test-integration:
	$(CARGO) test --test '*' -- --nocapture

lint:
	$(CARGO) clippy --all-targets -- -D warnings

check:
	$(CARGO) check --all-targets

run:
	$(CARGO) run -- serve

fmt:
	$(CARGO) fmt

clean:
	$(CARGO) clean

coverage:
	cargo llvm-cov --html --output-dir ./coverage_report
	@echo "Coverage HTML report generated at ./coverage_report/html/index.html"

coverage-open:
	cargo llvm-cov --html --open
