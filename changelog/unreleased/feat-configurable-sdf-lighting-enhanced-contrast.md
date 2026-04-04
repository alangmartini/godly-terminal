### Fixed

- **Disable enhanced contrast boost for browser text weight parity** — Set `ENHANCED_CONTRAST` from 0.5 to 0.0 in atlas shader so native text weight exactly matches browser rendering, which uses raw DirectWrite glyph coverage without shader-side boosting.
