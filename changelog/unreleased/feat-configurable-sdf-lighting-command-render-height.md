### Fixed

- **Command block render height includes border** - The command block's rendered rectangle now uses 32px height (matching CSS `fontSize 12 * lineHeight 1.5 + padding 12 + border 2`) instead of 30px, and text y-offset accounts for the 1px top border.
