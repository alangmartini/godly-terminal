### Fixed

- **Fix staging isolation broken by native frontend mode** — The `GODLY_INSTANCE=staging` env var was set inside `lib::run()`, but the native frontend path in `main.rs` spawns `godly-native.exe` and exits without calling `run()`. The child process inherited no instance var, causing it to connect to the production daemon instead of the staging one. Moved the env var setup to `main()` before the frontend mode dispatch.
