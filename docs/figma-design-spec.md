# Godly Terminal - Design Specification

Complete design spec for recreating the Godly Terminal UI in Figma.

---

## Color Tokens

### UI Chrome (Tokyo Night - Default)

| Token | Hex | Usage |
|-------|-----|-------|
| `bg-primary` | `#1a1b26` | Main background, terminal area |
| `bg-secondary` | `#16161e` | Sidebar, tab bar, dialogs |
| `bg-tertiary` | `#292e42` | Hover states, inactive elements |
| `bg-active` | `#33467c` | Active/focused elements |
| `text-primary` | `#a9b1d6` | Default text |
| `text-secondary` | `#565f89` | Dimmed/secondary text |
| `text-active` | `#c0caf5` | Bright/active text |
| `accent` | `#7aa2f7` | Primary blue accent |
| `accent-hover` | `#89b4fa` | Lighter blue on hover |
| `border-color` | `#292e42` | 1px borders |
| `danger` | `#f7768e` | Red (destructive actions) |
| `success` | `#9ece6a` | Green (success states) |

### Terminal ANSI Palette (Tokyo Night)

| Color | Normal | Bright |
|-------|--------|--------|
| Black | `#15161e` | `#414868` |
| Red | `#f7768e` | `#f7768e` |
| Green | `#9ece6a` | `#9ece6a` |
| Yellow | `#e0af68` | `#e0af68` |
| Blue | `#7aa2f7` | `#7aa2f7` |
| Magenta | `#bb9af7` | `#bb9af7` |
| Cyan | `#7dcfff` | `#7dcfff` |
| White | `#a9b1d6` | `#c0caf5` |

**Special:** Cursor `#c0caf5`, Selection `#283457`, Background `#1a1b26`, Foreground `#c0caf5`

### Dusk Theme (Alternative)

| Token | Hex |
|-------|-----|
| `bg-primary` | `#1d1f21` |
| `bg-secondary` | `#181a1b` |
| `bg-tertiary` | `#2e2b28` |
| `bg-active` | `#3d3733` |
| `text-primary` | `#c5c0b6` |
| `text-secondary` | `#5a564f` |
| `text-active` | `#d8d3c9` |
| `accent` | `#d4a96a` |
| `accent-hover` | `#e0be88` |
| `border-color` | `#2e2b28` |
| `danger` | `#c27070` |
| `success` | `#8fad7e` |

---

## Typography

### Font Families
- **UI Font:** `'Segoe UI', Tahoma, Geneva, Verdana, sans-serif`
- **Monospace Font:** `'Cascadia Code', Consolas, monospace`

### Font Scale

| Usage | Size | Weight | Extra |
|-------|------|--------|-------|
| Dialog titles | 14px | 600 | - |
| Body text / inputs | 13px | 400 | - |
| Tab titles | 12px | 400 | - |
| Section headers | 11px | 600 | `UPPERCASE`, letter-spacing 0.5px |
| Badges / small labels | 10-11px | 400 | - |
| WSL badge | 9px | 400 | - |
| Monospace (code, git) | 12px | 400 | `Cascadia Code` |

---

## Spacing System

| Value | Usage |
|-------|-------|
| 4px | Minimal (separators, radio buttons) |
| 6px | Small (context menu items, badges) |
| 8px | Standard padding (buttons, list items) |
| 12px | Component padding (headers, dialogs) |
| 16px | Section spacing, grid gaps |
| 20px | Dialog padding |

### Border Radius

| Value | Usage |
|-------|-------|
| 3px | Small (WSL badge, worktree status) |
| 4px | Medium (inputs, buttons, ghost) |
| 6px | Large (context menu, toast) |
| 8px | XL (dialogs, file drop indicator) |
| 10px | Pill (workspace count badge) |

---

## Layout Dimensions

| Element | Dimension |
|---------|-----------|
| Sidebar width | 200px |
| Tab bar height | 35px |
| Tab min-width | 120px |
| Tab max-width | 200px |
| Add-tab button | 35x35px |
| Close button | 20x20px |
| Split divider | 4px |
| Dialog min-width | 300px |
| Dialog max-width | 400px |
| Settings dialog | 560-640px wide, 80vh max-height |
| Toast width | 240-340px |
| Context menu min-width | 150px |

---

## Component Specs

### 1. Overall Layout

```
+-------+-----------------------------------+
|       |  Tab Bar (35px)                    |
|  Side |------------------------------------+
|  bar  |                                    |
| (200) |  Terminal Area (flex: 1)           |
|       |                                    |
|       |                                    |
+-------+------------------------------------+
```

- Root: `display: flex; height: 100vh; overflow: hidden`
- Sidebar: `flex-shrink: 0; width: 200px`
- Main content: `flex: 1; display: flex; flex-direction: column`

### 2. Sidebar

**Background:** `bg-secondary`
**Border:** 1px right, `border-color`

**Header:**
- Text: "WORKSPACES"
- Font: 11px, uppercase, weight 600, letter-spacing 0.5px
- Color: `text-secondary`
- Padding: 12px
- Border-bottom: 1px `border-color`

**Workspace Item:**
- Padding: 8px 12px
- Hover: background `bg-tertiary`
- Active: left-border 2px `accent`, text `text-active`
- WSL badge: 9px, orange (`#f97316`) bg, white text, 3px radius, padding 1px 4px
- Count badge: 10px radius pill, 11px, `bg-tertiary` + `text-secondary` (active: `accent` bg + white text)

**Bottom Buttons:**
- Padding: 8px 12px
- Color: `text-secondary`
- Border-top: 1px `border-color`
- Hover: `bg-tertiary`

### 3. Tab Bar

**Background:** `bg-secondary`
**Height:** 35px
**Border:** 1px bottom, `border-color`

**Tab (inactive):**
- Padding: 0 12px, min 120px, max 200px
- Background: `bg-secondary`
- Right border: 1px `border-color`
- Hover: `bg-tertiary`
- Close button: 20x20px, hidden (shown on hover), danger on hover

**Tab (active):**
- Background: `bg-primary`
- Top border: 2px `accent`
- Bottom border: none (blends with terminal area)
- Text: `text-active`

**Add Tab Button:**
- 35x35px centered
- "+" text, `text-secondary`
- Hover: `bg-tertiary`, `text-primary`

### 4. Terminal Area

**Background:** `bg-primary`
**Content:** Canvas2D rendering area

**Split Mode:**
- Horizontal: `flex-direction: row`
- Vertical: `flex-direction: column`
- Divider: 4px, `bg-tertiary`, hover `accent`
- Focused pane: top 2px `accent` border

**File Drop Indicator:**
- Dashed 2px `accent` border
- Background: `rgba(122, 162, 247, 0.08)`
- 8px inset, 8px radius

### 5. Dialogs

**Overlay:** fixed, full-screen, `rgba(0, 0, 0, 0.5)`

**Dialog Box:**
- Background: `bg-secondary`
- Border: 1px `border-color`
- Radius: 8px
- Padding: 20px
- Width: 300-400px

**Title:** 14px, weight 600, margin-bottom 16px
**Input:** full-width, `bg-primary`, 1px border, 4px radius, 13px, focus: `accent` border
**Primary Button:** `accent` bg, white text, 6px 16px padding, 4px radius
**Secondary Button:** `bg-tertiary`, `text-primary`, same sizing

### 6. Settings Dialog

**Size:** 560-640px wide, 80vh max-height

**Tab Navigation:**
- Horizontal below header
- 12px font, 8px 16px padding
- Active: `text-active`, bottom 2px `accent`
- Inactive: `text-secondary`

**Theme Cards:**
- Grid layout, 260px min column
- 2px border, 8px radius, `bg-primary`
- Hover: border `text-secondary`, translateY(-1px)
- Selected: border `accent`
- Name: 13px, weight 600, `text-active`
- Description: 12px, `text-secondary`

**Shortcut Rows:**
- Label: 13px, flex: 1
- Binding: 12px monospace, `bg-primary`, 1px border, 4px radius, 4px 10px padding

### 7. Context Menu

**Background:** `bg-secondary`
**Border:** 1px `border-color`
**Radius:** 6px
**Shadow:** `0 4px 16px rgba(0, 0, 0, 0.4)`
**Backdrop:** `blur(8px)`
**Padding:** 4px 0

**Menu Item:** 6px 12px padding, flex, center-aligned, 8px gap
**Hover:** `bg-active`
**Danger Hover:** `danger` bg, white text
**Separator:** 1px height, `border-color`, 4px vertical margin

### 8. Toast Notifications

**Position:** Fixed, bottom-right, 12px from edges
**Layout:** Column-reverse (newest on top), 8px gap

**Toast:**
- Background: `bg-secondary`
- Border: 1px `border-color`, left 3px `accent`
- Radius: 6px, padding 10px 14px
- Shadow: `0 4px 12px rgba(0, 0, 0, 0.4)`
- Title: weight 600, `text-active`, 13px
- Body: `text-secondary`, 12px
- Animation: slide-in from right (0.25s)

### 9. Worktree Panel

**Position:** Bottom of sidebar
**Header:** 8px 12px padding, 11px uppercase, weight 600, `text-secondary`
**Toggle:** Chevron ("▸" collapsed / "▾" expanded)

**Worktree Item:**
- 4px padding, 8px gap
- Branch name: 12px
- Commit hash: 10px monospace
- Actions on hover: Open (accent hover), Delete (danger hover)

---

## Interactive States

### Transitions
| Speed | Usage |
|-------|-------|
| 0.1s | Hover/background changes |
| 0.15s | Color/border transitions |
| 0.25s | Toast slide-in |
| 0.3s | Toast fade-out |

### Animations
- **Notification pulse:** 1.5s infinite (opacity 1 -> 0.4 -> 1)
- **Refresh spinner:** 0.8s linear infinite (360deg rotation)
- **Binding capture pulse:** 1s (border color pulses)

### Drag & Drop
- **Dragging element:** 0.5 opacity
- **Drag ghost:** `bg-secondary`, 1px `accent` border, 4px radius, 0.8 opacity
- **Drop target (tab):** left 2px `accent` border
- **Drop target (workspace):** `accent` background

---

## Scrollbar Styling

- Width: 8px
- Track: `bg-secondary`
- Thumb: `border-color`, 8px radius
- Thumb hover: `text-secondary`
- Tab bar scrollbar: 3px height (thin)

---

## Icons

The app uses **Unicode characters** exclusively (no SVG or icon fonts):

| Icon | Character | Usage |
|------|-----------|-------|
| Add | `+` | New tab, new workspace |
| Close | `x` | Close tab, dismiss dialog |
| Expand | `▾` | Dropdown expanded |
| Collapse | `▸` | Dropdown collapsed |
| Figma | `◆` | Figma tab indicator |

---

## Figma Frame Recommendations

When building in Figma, create these frames:

1. **Full App (1280x800)** - Complete application layout
2. **Sidebar (200x800)** - Isolated sidebar with all states
3. **Tab Bar (1080x35)** - Tab bar with active/inactive/hover states
4. **Terminal Area (1080x765)** - Terminal with sample content
5. **Settings Dialog (640x600)** - Settings with theme cards
6. **Context Menu (180x auto)** - Right-click menu
7. **Toast Stack (340x auto)** - Notification examples
8. **Dialog (400x auto)** - Standard dialog
9. **Split View (1080x765)** - Horizontal and vertical splits
10. **Component States** - Button, tab, workspace item in all states
