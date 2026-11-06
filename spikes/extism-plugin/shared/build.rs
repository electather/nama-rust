fn main() {
    buffa_build::Config::new()
        .files(&["../proto/spike.proto"])
        .includes(&["../proto"])
        .include_file("protos.rs")
        .compile()
        .expect("compile spike.proto (needs protoc on PATH)");
}
