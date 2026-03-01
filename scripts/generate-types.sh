#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "Generating TypeScript types from Rust protocol..."
cd src-tauri
cargo test -p godly-protocol export_bindings -- --ignored 2>/dev/null || true
mkdir -p ../src/generated
if [ -d "protocol/bindings" ]; then
  cp protocol/bindings/*.ts ../src/generated/
  echo "Types generated in src/generated/"
  ls ../src/generated/*.ts 2>/dev/null
else
  echo "No bindings directory found. ts-rs may export elsewhere."
  find . -name "*.ts" -path "*/bindings/*" 2>/dev/null
fi
