### Fixed

- **Terminal text invisible after typing** — The `stream://` custom protocol URLs used for real-time terminal updates never worked on Windows (WebView2 requires `http://{scheme}.localhost/` format). Changed `stream://localhost/` to `http://stream.localhost/` so diff and output streams actually connect. Fixes #486.
