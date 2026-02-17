# Figma Design Skill

Create and modify UI designs in Figma for Godly Terminal using the Playwright + Plugin API approach. This bypasses Figma MCP tool call limits entirely.

## Usage

```
/figma-design <figma-url> [description of what to create]
```

Examples:
- `/figma-design https://www.figma.com/design/abc123/File?node-id=0-1 Create the full app layout`
- `/figma-design https://www.figma.com/design/abc123/File?node-id=0-1 Add a new search dialog component`
- `/figma-design https://www.figma.com/design/abc123/File?node-id=12-2 Update the sidebar with a new panel`

## Instructions

### IMPORTANT: Use Playwright, NOT the Figma MCP

**Never use `mcp__plugin_figma_figma__*` tools for creating or modifying designs.** Those tools are read-only and have tool call limits on Free plans. Instead:

1. Use `mcp__plugin_playwright_playwright__*` tools to open the Figma file in a browser
2. Use `browser_evaluate` to call `window.figma` (the Figma Plugin API) for all creation/modification
3. Use Playwright `browser_take_screenshot` for visual verification

The Figma MCP tools (`get_screenshot`, `get_metadata`, `get_design_context`) are OK for **reading** existing designs, but budget them carefully — Free tier has strict limits.

### Step 1: Navigate to the Figma file

```
Use mcp__plugin_playwright_playwright__browser_navigate to open the Figma URL.
```

### Step 2: Wait for page load and check auth

```
Wait 3 seconds, then take a snapshot.
```

Check the snapshot for:
- **"Crie sua conta" / "Sign up"** banner = NOT logged in. Ask the user to log in via the Playwright browser window.
- **Left sidebar with "Layers" panel** = logged in with edit access. Proceed.

If there's a cookie consent banner, dismiss it first by clicking the accept button.

### Step 3: Verify Plugin API access

Run this check before doing anything else:

```javascript
// browser_evaluate
() => {
  return {
    hasFigma: typeof window.figma !== 'undefined',
    hasCreateFrame: typeof window.figma?.createFrame === 'function',
    hasCurrentPage: window.figma?.currentPage !== undefined,
    pageChildren: window.figma?.currentPage?.children?.length ?? 0
  };
}
```

If `hasFigma` is false, the page hasn't fully loaded or the user isn't logged in. Wait and retry.

### Step 4: Create designs using the Plugin API

Use `browser_evaluate` with async functions that call `window.figma.*` methods.

#### Helper functions (include in every evaluate call)

```javascript
function hexToRgb(hex) {
  return {
    r: parseInt(hex.slice(0, 2), 16) / 255,
    g: parseInt(hex.slice(2, 4), 16) / 255,
    b: parseInt(hex.slice(4, 6), 16) / 255
  };
}

function solidFill(hex) {
  return [{ type: 'SOLID', color: hexToRgb(hex) }];
}
```

#### Load fonts before creating text

**Always** load fonts before creating text nodes. Available fonts on Figma:
- `{ family: "Inter", style: "Regular" }` — UI text
- `{ family: "Inter", style: "Medium" }` — medium weight
- `{ family: "Inter", style: "Semi Bold" }` — headings, labels
- `{ family: "Inter", style: "Bold" }` — bold text
- `{ family: "Roboto Mono", style: "Regular" }` — monospace/terminal
- `{ family: "Roboto Mono", style: "Bold" }` — bold monospace

```javascript
await figma.loadFontAsync({ family: "Inter", style: "Regular" });
await figma.loadFontAsync({ family: "Inter", style: "Semi Bold" });
await figma.loadFontAsync({ family: "Roboto Mono", style: "Regular" });
```

#### Key Plugin API methods

```javascript
// Create elements
const frame = figma.createFrame();
const rect = figma.createRectangle();
const text = figma.createText();
const ellipse = figma.createEllipse();

// Set properties
frame.name = "My Frame";
frame.resize(width, height);
frame.x = 0;
frame.y = 0;
frame.fills = solidFill('1a1b26');
frame.clipsContent = true;
frame.cornerRadius = 8;

// Borders (strokes)
frame.strokes = solidFill('292e42');
frame.strokeWeight = 1;
frame.strokeAlign = 'INSIDE';
// Individual stroke sides
frame.strokeTopWeight = 0;
frame.strokeBottomWeight = 1;
frame.strokeLeftWeight = 0;
frame.strokeRightWeight = 0;

// Shadows
frame.effects = [{
  type: 'DROP_SHADOW',
  color: { r: 0, g: 0, b: 0, a: 0.4 },
  offset: { x: 0, y: 4 },
  radius: 16,
  visible: true,
  blendMode: 'NORMAL'
}];

// Text properties
text.characters = "Hello World";
text.fontSize = 13;
text.fontName = { family: "Inter", style: "Regular" };
text.fills = solidFill('a9b1d6');
text.letterSpacing = { value: 0.5, unit: 'PIXELS' };

// Hierarchy
parentFrame.appendChild(childFrame);
figma.currentPage.appendChild(topLevelFrame);

// Navigate viewport
figma.viewport.scrollAndZoomIntoView(figma.currentPage.children);
figma.viewport.scrollAndZoomIntoView([specificNode]);
```

### Step 5: Verify with screenshots

After creating elements, verify using Playwright screenshots:

```
Use mcp__plugin_playwright_playwright__browser_take_screenshot to capture the viewport.
```

To zoom into a specific frame first:
```javascript
// browser_evaluate
() => {
  const target = figma.currentPage.children.find(c => c.name === "My Frame");
  if (target) figma.viewport.scrollAndZoomIntoView([target]);
  return 'zoomed';
}
```

Wait 1 second after zooming before taking a screenshot.

### Step 6: Inventory check

After all work is done, run an inventory check:

```javascript
// browser_evaluate
() => {
  return figma.currentPage.children.map(c => ({
    name: c.name,
    type: c.type,
    width: Math.round(c.width),
    height: Math.round(c.height),
    childCount: c.children ? c.children.length : 0
  }));
}
```

## Godly Terminal Design Tokens

Always use these exact color values from the Tokyo Night theme:

### Backgrounds
| Token | Hex | Usage |
|-------|-----|-------|
| bg-primary | `1a1b26` | Main background, terminal area |
| bg-secondary | `16161e` | Sidebar, tab bar, dialogs |
| bg-tertiary | `292e42` | Hover states, inactive surfaces |
| bg-active | `33467c` | Active/focused elements |

### Text
| Token | Hex | Usage |
|-------|-----|-------|
| text-primary | `a9b1d6` | Default text |
| text-secondary | `565f89` | Dimmed/secondary text |
| text-active | `c0caf5` | Bright/active text |

### Accents
| Token | Hex | Usage |
|-------|-----|-------|
| accent | `7aa2f7` | Primary blue (active borders, buttons) |
| accent-hover | `89b4fa` | Lighter blue on hover |
| border-color | `292e42` | All 1px borders |
| danger | `f7768e` | Red (destructive actions) |
| success | `9ece6a` | Green (success states) |

### Terminal ANSI Colors
| Color | Hex | Usage |
|-------|-----|-------|
| blue | `7aa2f7` | Prompts, paths |
| green | `9ece6a` | Success, compile output |
| yellow | `e0af68` | Warnings, running commands |
| cyan | `7dcfff` | Info messages |
| red | `f7768e` | Errors |
| magenta | `bb9af7` | Special highlights |
| white | `a9b1d6` | Default terminal text |
| bright-white | `c0caf5` | Bright terminal text, cursor |

### Special
| Token | Hex | Usage |
|-------|-----|-------|
| wsl-badge | `f97316` | Orange WSL badge background |
| cursor | `c0caf5` | Terminal cursor block |
| selection | `283457` | Selection highlight |

## Typography Reference

| Usage | Font | Size | Weight |
|-------|------|------|--------|
| Dialog titles | Inter | 14px | Semi Bold |
| Body text | Inter | 13px | Regular |
| Tab titles | Inter | 12px | Regular |
| Section headers | Inter | 11px | Semi Bold, UPPERCASE, 0.5px letter-spacing |
| Badges | Inter | 10-11px | Medium |
| WSL badge | Inter | 9px | Semi Bold |
| Terminal text | Roboto Mono | 13px | Regular |
| Code/bindings | Roboto Mono | 12px | Regular |

## Layout Dimensions

| Element | Dimension |
|---------|-----------|
| Full app | 1280 x 800 |
| Sidebar | 200px wide, full height |
| Tab bar | remaining width, 35px tall |
| Terminal area | remaining width, remaining height |
| Tab | min 120px, max 200px wide |
| Add tab button | 35 x 35px |
| Workspace item | 200 x 36px |
| Dialog | 300-400px wide, 8px corner radius |
| Settings dialog | 560-640px wide |
| Context menu | ~180px wide, 6px corner radius |
| Toast | 240-340px wide, 6px corner radius |
| Split divider | 4px |

## Common Component Recipes

### Active tab indicator
```javascript
const indicator = figma.createRectangle();
indicator.resize(tabWidth, 2);
indicator.x = 0;
indicator.y = 0;
indicator.fills = solidFill('7aa2f7');
tabFrame.appendChild(indicator);
```

### Sidebar active workspace
```javascript
// Left border accent
const bar = figma.createRectangle();
bar.resize(2, itemHeight);
bar.x = 0;
bar.y = 0;
bar.fills = solidFill('7aa2f7');
item.appendChild(bar);
// Text in text-active color
nameText.fills = solidFill('c0caf5');
```

### Pill badge
```javascript
const badge = figma.createFrame();
badge.resize(20, 18);
badge.fills = solidFill('7aa2f7'); // active, or '292e42' inactive
badge.cornerRadius = 10;
```

### Dialog buttons
```javascript
// Primary button
const btn = figma.createFrame();
btn.fills = solidFill('7aa2f7');
btn.cornerRadius = 4;
// text: white (ffffff), 13px Inter Regular, padding 6px 16px

// Secondary button
const btn2 = figma.createFrame();
btn2.fills = solidFill('292e42');
btn2.cornerRadius = 4;
// text: text-primary (a9b1d6)
```

### Toast with accent border
```javascript
const toast = figma.createFrame();
toast.fills = solidFill('16161e');
toast.cornerRadius = 6;
toast.strokes = solidFill('292e42');
toast.strokeWeight = 1;
toast.effects = [{ type: 'DROP_SHADOW', color: { r:0, g:0, b:0, a:0.4 }, offset: { x:0, y:4 }, radius: 12, visible: true, blendMode: 'NORMAL' }];
// Left accent: 3px wide rectangle, fills accent blue
```

## Gotchas

- **Font loading is required** before setting `text.characters`. If you forget `loadFontAsync`, the call will throw.
- **`evaluate` timeout**: For large designs, Playwright may timeout. Split creation into multiple `browser_evaluate` calls.
- **Canvas is WebGL**: Playwright snapshots (`browser_snapshot`) won't show canvas content. Use `browser_take_screenshot` for visual verification.
- **Zoom after creation**: Always call `figma.viewport.scrollAndZoomIntoView()` then wait 1s before screenshotting.
- **Figma autosaves**: Changes are automatically saved. No need to manually save.
- **String escaping in evaluate**: Backslashes in strings need double-escaping in JS template strings (e.g., `"C:\\\\Users"` for `C:\Users`).

## Full Design Spec Reference

See `docs/figma-design-spec.md` for the complete component specifications and `figma/design-system-rules.md` for Figma-to-code conventions.
