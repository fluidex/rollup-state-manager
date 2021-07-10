lint:
	cargo fmt --all -- --check
	cargo clippy --features persist_sled -- -D warnings
