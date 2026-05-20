@echo off
set AWS_PROFILE=irisawamasaru
cargo run --manifest-path %~dp0tester\Cargo.toml --bin create-stack
