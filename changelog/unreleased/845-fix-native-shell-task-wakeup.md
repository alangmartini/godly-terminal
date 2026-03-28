### Fixed
- **Native shell screen refresh latency** - Background terminal tasks now wake the Win32 event loop when they finish, and the native shell keepalive timer now fires every 100ms instead of every second so grid updates and new-terminal prompts render promptly without waiting for unrelated input. (#845)
