#!/bin/bash
# Build godly-whisper with CUDA support.
# Centralizes the CUDA environment variables so they aren't duplicated across npm scripts.
# Usage: scripts/cargo-whisper.sh [extra cargo args, e.g. --release]

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

if command -v cygpath >/dev/null 2>&1; then
  resolver_path="$(cygpath -w "$script_dir/find-msvc-tool.ps1")"
else
  resolver_path="$(cd "$script_dir" && pwd -W)\\find-msvc-tool.ps1"
fi

export CMAKE_GENERATOR=Ninja
export CUDAHOSTCXX="$(powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$resolver_path" -Tool cl | tr -d '\r')"
export CMAKE_CUDA_ARCHITECTURES=89
export CXXFLAGS=/std:c++17
export CUDAFLAGS=-std=c++17

exec cargo build -p godly-whisper --features cuda "$@"
