### Fixed

- URL detector no longer panics when multi-byte UTF-8 characters appear in terminal grid text. The character iteration now advances the byte index by the full UTF-8 character width instead of always advancing by 1 byte, preventing string boundary violations. [#617](https://github.com/alangmartini/godly-terminal/issues/617)
