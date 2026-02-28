### Fixed

- **Whisper test recording shows [object Object]** — The "Test Recording" button in settings and the Quick Claude voice input displayed `[object Object]` instead of the transcribed text because `whisperStopRecording()` returns a `TranscriptionResult` object, not a string. (#442)

### Tests

- **Fixed whisper test mocks** — Test mocks for `whisperStopRecording` now return `TranscriptionResult { text, durationMs }` matching the real return type. Added regression test that exercises the test recording button and verifies the displayed text.
