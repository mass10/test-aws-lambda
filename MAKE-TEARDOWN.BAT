@echo off
set AWS_PROFILE=irisawamasaru
cargo run --manifest-path %~dp0Cargo.toml --bin teardown
