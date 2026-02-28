#!/bin/bash
# Build godly-whisper with CUDA support.
# Centralizes the CUDA environment variables so they aren't duplicated across npm scripts.
# Usage: scripts/cargo-whisper.sh [extra cargo args, e.g. --release]

export CMAKE_GENERATOR=Ninja
export CUDAHOSTCXX="C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64/cl.exe"
export CMAKE_CUDA_ARCHITECTURES=89
export CXXFLAGS=/std:c++17
export CUDAFLAGS=-std=c++17

exec cargo build -p godly-whisper --features cuda "$@"
