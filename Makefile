lint:
	cargo fmt --all -- --check
	cargo clippy --features persist_sled -- -D warnings

genpb:
	cd proto && protoc -Ithird_party/googleapis -I. --include_imports --include_source_info --descriptor_set_out=rollup_state.pb rollup_state.proto
