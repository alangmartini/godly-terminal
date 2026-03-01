### Fixed
- **Gemini 3 empty content parse failure** — Branch Name AI no longer fails when gemini-3-flash-preview returns `"content": {}` (empty object with no `parts` field) after spending all tokens on thinking
