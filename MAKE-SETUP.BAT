@echo off
set AWS_PROFILE=irisawamasaru
cd /d %~dp0tester
cargo run --bin create-stack
