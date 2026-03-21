### Fixed
- **Dead key duplicate character bug** — pressing dead key (e.g. `'` on US-International) then backspace now correctly deletes one character instead of creating a duplicate quote. The fix prevents premature character dispatch when text composition is pending.
