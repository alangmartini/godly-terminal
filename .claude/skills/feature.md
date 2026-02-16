# Feature Skill

Implement new terminal features across both the TypeScript frontend and Rust backend.

## Usage

```
/feature <feature-name> [description]
```

## Instructions

When implementing a new feature in Godly Terminal, you typically need to modify both the frontend and backend.

### Architecture Overview

```
Frontend (TypeScript)          Backend (Rust/Tauri)
─────────────────────         ──────────────────────
src/components/         ──►   src-tauri/src/commands/
src/services/           ──►   (Tauri invoke)
src/state/store.ts            src-tauri/src/state/
```

### Implementation Checklist

#### 1. Backend (Rust) - if needed

**Add new command** in `src-tauri/src/commands/`:

```rust
#[tauri::command]
pub async fn my_new_command(
    state: tauri::State<'_, AppState>,
    param: String,
) -> Result<ReturnType, String> {
    // Implementation
}
```

**Register command** in `src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::my_new_command,
])
```

**Add state** if needed in `src-tauri/src/state/models.rs` and `app_state.rs`.

#### 2. Frontend Service

**Add service method** in `src/services/`:

```typescript
export async function myNewCommand(param: string): Promise<ReturnType> {
  return invoke<ReturnType>('my_new_command', { param });
}
```

#### 3. State Management

**Add state** in `src/state/store.ts`:

```typescript
interface AppState {
  // ... existing state
  newFeatureData: FeatureType;
}
```

#### 4. UI Component

**Update or create component** in `src/components/`:

```typescript
// Subscribe to state changes
store.subscribe((state) => {
  // Update UI based on new state
});

// Call service
import { myNewCommand } from '../services/my-service';
await myNewCommand(param);
```

#### 5. Styling

**Add styles** in `src/styles/main.css`.

### Key Files by Area

| Area | Frontend | Backend |
|------|----------|---------|
| Terminal I/O | `TerminalPane.ts`, `terminal-service.ts` | `commands/terminal.rs`, `pty/manager.rs` |
| Workspaces | `WorkspaceSidebar.ts`, `workspace-service.ts` | `commands/workspace.rs` |
| Tabs | `TabBar.ts` | N/A (frontend only) |
| State | `state/store.ts` | `state/app_state.rs`, `state/models.rs` |
| Persistence | N/A | `persistence/layout.rs` |

### Event Communication (Backend → Frontend)

For real-time updates from backend to frontend:

```rust
// Rust - emit event
app_handle.emit("event-name", payload)?;
```

```typescript
// TypeScript - listen for event
import { listen } from '@tauri-apps/api/event';

await listen<PayloadType>('event-name', (event) => {
  // Handle event
});
```

### Testing the Feature

1. Run `cd godly-terminal && npm run tauri dev`
2. Test the feature manually
3. Check browser console for frontend errors
4. Check terminal for Rust backend errors
