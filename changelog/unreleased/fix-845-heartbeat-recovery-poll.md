### Fixed

- **Terminal refresh regression** — Restored responsive terminal rendering by replacing the subscription-staleness gate with an always-on 100ms heartbeat timer; the grid fingerprint check and fetching guard prevent the old render loop without starving the display (#845)
