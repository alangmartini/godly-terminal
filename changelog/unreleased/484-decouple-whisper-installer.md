### Changed
- **Voice/Whisper decoupled from main installer** — godly-whisper is no longer built or bundled with the main app. Users who want voice-to-text can download a separate standalone installer from GitHub Releases. The main app gracefully hides the mic button when whisper is not installed and shows an install prompt in Settings. (#484)
