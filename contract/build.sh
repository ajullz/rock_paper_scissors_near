#!/bin/sh

echo ">> Building contract"

rustup target add wasm32-unknown-unknown
cargo build -p game --target wasm32-unknown-unknown --release
cargo build -p factory --target wasm32-unknown-unknown --release
