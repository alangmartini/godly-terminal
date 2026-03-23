### Fixed
- **Phone Remote health check on Windows** — fixed WSAEADDRNOTAVAIL error when health check tried to connect to `0.0.0.0:3377`. The socket bind accepts `0.0.0.0` (all interfaces), but Windows doesn't support connecting to it; now maps to loopback `127.0.0.1` for the health check connection.
