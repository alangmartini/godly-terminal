# Component Skill

Generate new UI components following the Godly Terminal component pattern.

## Usage

```
/component <ComponentName> [description]
```

## Instructions

Create a new TypeScript component in `godly-terminal/src/components/` following the existing patterns.

### Component Pattern

Components in this codebase follow this structure:

```typescript
// godly-terminal/src/components/ComponentName.ts

import { store } from '../state/store';

export class ComponentName {
  private container: HTMLElement;
  private unsubscribe: (() => void) | null = null;

  constructor(parent: HTMLElement) {
    this.container = document.createElement('div');
    this.container.className = 'component-name';
    parent.appendChild(this.container);

    this.render();
    this.setupEventListeners();
    this.subscribeToStore();
  }

  private render(): void {
    this.container.innerHTML = `
      <!-- Component HTML structure -->
    `;
  }

  private setupEventListeners(): void {
    // Add DOM event listeners
  }

  private subscribeToStore(): void {
    this.unsubscribe = store.subscribe((state) => {
      // React to state changes
    });
  }

  public destroy(): void {
    if (this.unsubscribe) {
      this.unsubscribe();
    }
    this.container.remove();
  }
}
```

### Key Patterns to Follow

1. **Class-based components** - Each component is a TypeScript class
2. **Parent injection** - Constructor takes a parent HTMLElement
3. **Store subscription** - Subscribe to the store for reactive updates
4. **Cleanup method** - `destroy()` method for cleanup

### Existing Components Reference

- `App.ts` - Main orchestrator, manages state and child components
- `TerminalPane.ts` - xterm.js wrapper with PTY I/O
- `TabBar.ts` - Tab management with drag-and-drop
- `WorkspaceSidebar.ts` - Workspace switcher

### Styling

Add component styles to `godly-terminal/src/styles/main.css`:

```css
/* Component: ComponentName */
.component-name {
  /* Component styles */
}
```

Use the existing CSS variables for theming consistency.

### After Creating

1. Import and instantiate in the appropriate parent component (usually `App.ts`)
2. Add necessary state to `store.ts` if the component needs new state
3. Create services in `services/` if the component needs Tauri IPC
