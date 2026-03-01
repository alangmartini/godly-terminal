### Fixed

- **Terminal text invisible until tab switch** — Fixed a race condition where binary diff stream data arriving during the initial snapshot fetch created a deadlock, leaving `cachedSnapshot` permanently null. The fix sets `forceFullFetch` at all three deadlock entry points (mount race, resize race, fetch retry) so recovery fetches bypass the `diffStreamActive` guard. ([#486](https://github.com/alangmartini/godly-terminal/pull/486))
