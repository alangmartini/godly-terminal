### Fixed
- **Quick Claude enter retry** — Quick Claude now verifies that Enter was processed by checking the terminal grid and retries up to 5 times if the prompt is still visible, fixing intermittent failures where the prompt sat unsubmitted (#506)
