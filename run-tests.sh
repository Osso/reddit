#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "=== Building ==="
cargo build

echo "=== Clippy ==="
cargo clippy -- -D warnings

echo "=== All checks passed ==="
