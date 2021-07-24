fn build_grpc() {
    tonic_build::configure()
        .out_dir("src/grpc/rpc")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile(&["proto/rollup_state.proto"], &["proto", "proto/third_party/googleapis"])
        .unwrap();
}

fn main() {
    build_grpc()
}
