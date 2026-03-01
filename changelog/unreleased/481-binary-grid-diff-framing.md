### Fixed

- **Binary framing for GridDiff on daemon→bridge pipe** — GridDiff events now use compact binary encoding (tag 0x04) instead of JSON serialization on the daemon→bridge named pipe, reducing per-diff payload by ~10x. The bridge also performs zero-copy forwarding of raw binary diffs to the stream:// protocol, eliminating redundant deserialize + re-encode cycles. Fixes low FPS (~18) and high input latency (p50=3.5s) caused by JSON serialization saturating the single-threaded bridge I/O loop. (PR #481)
