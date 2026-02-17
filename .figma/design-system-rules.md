# Godly Terminal - Design System Rules

Rules for integrating Figma designs with the Godly Terminal codebase.

---

## Framework & Tooling

- **Frontend:** TypeScript + vanilla DOM (no React/Vue/Angular)
- **Styling:** Single global CSS file (`src/styles/main.css`) with CSS custom properties
- **Rendering:** Canvas2D for terminal content (not HTML elements)
- **Build:** Vite (frontend) + Cargo workspace (Rust backend)
- **Platform:** Windows desktop via Tauri 2.0

## Token Mapping

All UI colors are defined as CSS custom properties on `:root`. When implementing a Figma design, map Figma color tokens to these variables:

```css
/* Background hierarchy (darkest to lightest) */
var(--bg-secondary)   /* Sidebar, tab bar, dialogs, cards */
var(--bg-primary)     /* Main content area, terminal */
var(--bg-tertiary)    /* Hover states, inactive surfaces */
var(--bg-active)      /* Active/focused interactive elements */

/* Text hierarchy */
var(--text-secondary) /* Dimmed, labels, placeholders */
var(--text-primary)   /* Default body text */
var(--text-active)    /* Emphasized, active, headings */

/* Semantic */
var(--accent)         /* Primary interactive color (blue) */
var(--accent-hover)   /* Hover state for accent */
var(--border-color)   /* All 1px borders */
var(--danger)         /* Destructive actions */
var(--success)        /* Success indicators */
```

### Theme Support

The app supports multiple themes. Never hardcode hex values — always use CSS variables. Themes are switched at runtime by mutating `document.documentElement.style`.

Built-in themes:
- **Tokyo Night** (default): Blue accent (#7aa2f7), dark navy background
- **Dusk**: Warm amber accent (#d4a96a), dark charcoal background

## Component Patterns

### DOM Structure

Components are vanilla TypeScript classes that create DOM elements manually:

```typescript
class MyComponent {
  private element: HTMLElement;

  constructor(container: HTMLElement) {
    this.element = document.createElement('div');
    this.element.className = 'my-component';
    container.appendChild(this.element);
  }
}
```

### CSS Class Naming

- **kebab-case** for all class names
- State classes: `.active`, `.dragging`, `.drag-over`, `.disabled`, `.editing`, `.capturing`
- No BEM, no CSS modules — simple flat class names

### Layout

- **Flexbox only** — no CSS Grid in the current codebase
- Sidebar + main content is horizontal flex
- Tab bar + terminal area is vertical flex within main content

### Sizing

Fixed dimensions for structural elements:
- Sidebar: `200px` wide
- Tab bar: `35px` tall
- Everything else uses `flex: 1` for fluid sizing

### Borders

- Always 1px solid `var(--border-color)`
- Active indicators: 2px solid `var(--accent)`
- Never use box-shadow for borders — real borders only

### Transitions

Standard: `0.1s` for background/color changes. Never exceed `0.3s` for any UI transition.

## Icon Convention

**Unicode characters only.** Do not introduce SVG icons or icon fonts. The codebase uses:
- `+` for add actions
- `x` for close/delete
- `▸` / `▾` for collapse/expand toggles

## Figma-to-Code Workflow

1. Export colors as CSS custom property values (not raw hex in components)
2. Export spacing as pixel values matching the 4/8/12/16/20px scale
3. Export typography using the system font stack (Segoe UI for UI, Cascadia Code for code)
4. Terminal content is Canvas2D — never convert terminal grids to HTML elements
5. All interactive states (hover, active, focus, disabled) must be defined per component

## File Paths

| What | Where |
|------|-------|
| Global CSS | `src/styles/main.css` |
| Theme definitions | `src/themes/builtin.ts` |
| Theme types | `src/themes/types.ts` |
| Theme store | `src/state/theme-store.ts` |
| Components | `src/components/*.ts` |
| Services | `src/services/*.ts` |
| State management | `src/state/*.ts` |

## Do NOT

- Add external CSS frameworks (Tailwind, Bootstrap, etc.)
- Use CSS-in-JS or styled-components
- Import icon libraries (FontAwesome, Lucide, etc.)
- Use CSS Grid for layout (keep consistency with flexbox)
- Add CSS preprocessors (SCSS, LESS)
- Hardcode color hex values outside of theme definitions
