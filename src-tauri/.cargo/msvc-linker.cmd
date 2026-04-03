@echo off
setlocal

for /f "usebackq delims=" %%I in (`powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0..\..\scripts\find-msvc-tool.ps1" -Tool linker`) do set "MSVC_LINKER=%%I"

if not defined MSVC_LINKER (
    echo Failed to resolve MSVC linker.>&2
    exit /b 1
)

"%MSVC_LINKER%" %*
exit /b %ERRORLEVEL%
