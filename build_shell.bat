@echo off
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvarsall.bat" x64
cd /d C:\Users\User\godly-terminal\src-tauri
cargo build --release -p godly-shell
