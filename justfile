default: fmt lint test

fmt:
	cargo fmt

lint:
	cargo clippy --all-targets --all-features -D warnings

test:
	cargo test

coverage:
	cargo llvm-cov --workspace --lcov --output-path coverage.lcov --fail-under-lines 85 --no-report

compliance command='':
	cargo run --release -p mcp_multi_tool --bin compliance -- --command {{command}}

security:
	cargo deny check advisories
