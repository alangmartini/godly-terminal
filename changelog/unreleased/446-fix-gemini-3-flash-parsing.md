### Fixed
- **Gemini 3 Flash Preview response parsing** — branch name AI no longer fails with "Failed to parse Gemini response" when using gemini-3-flash-preview. Handles optional candidate content (safety blocks, MAX_TOKENS), thinking parts, string error responses, and non-JSON HTTP errors (#446)
