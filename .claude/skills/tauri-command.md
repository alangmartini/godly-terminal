# Tauri Command Skill

Add new Tauri IPC commands with both Rust handler and TypeScript service.

## Usage

```
/tauri-command <command_name> [description]
```

## Instructions

This skill creates the full stack for a new Tauri command: Rust handler + TypeScript service.

### Step 1: Create Rust Command

Add to the appropriate file in `src-tauri/src/commands/` (or create a new module):

```rust
use tauri::State;
use crate::state::AppState;

#[tauri::command]
pub async fn command_name(
    state: State<'_, AppState>,
    // Add parameters as needed
    param1: String,
    param2: Option<i32>,
) -> Result<ReturnType, String> {
    // Implementation
    Ok(result)
}
```

### Step 2: Register Command

In `src-tauri/src/lib.rs`, add to the invoke handler:

```rust
.invoke_handler(tauri::generate_handler![
    // Existing commands...
    commands::terminal::create_terminal,
    commands::terminal::write_to_terminal,
    // ...

    // Add your new command
    commands::module::command_name,
])
```

### Step 3: Create TypeScript Service

Add to appropriate service file in `src/services/` or create new:

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface CommandResult {
  // Define the return type
}

export async function commandName(
  param1: string,
  param2?: number
): Promise<CommandResult> {
  return invoke<CommandResult>('command_name', {
    param1,
    param2,
  });
}
```

### Parameter Naming Convention

Tauri automatically converts between:
- Rust `snake_case` ↔ TypeScript `camelCase`

So `terminal_id` in Rust becomes `terminalId` in the TypeScript invoke call.

### Common Patterns

#### Accessing PTY Sessions

```rust
#[tauri::command]
pub async fn terminal_operation(
    state: State<'_, AppState>,
    terminal_id: String,
) -> Result<(), String> {
    let manager = state.pty_manager.lock();
    let session = manager.get_session(&terminal_id)
        .ok_or("Terminal not found")?;
    // Use session...
    Ok(())
}
```

#### Emitting Events

```rust
#[tauri::command]
pub async fn command_with_event(
    app: tauri::AppHandle,
    // ...
) -> Result<(), String> {
    app.emit("event-name", payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

#### With Window Handle

```rust
#[tauri::command]
pub async fn window_operation(
    window: tauri::Window,
    // ...
) -> Result<(), String> {
    // window operations
    Ok(())
}
```

### Existing Commands Reference

**Terminal commands** (`commands/terminal.rs`):
- `create_terminal` - Create new PTY session
- `write_to_terminal` - Write input to PTY
- `resize_terminal` - Resize PTY
- `close_terminal` - Close PTY session

**Workspace commands** (`commands/workspace.rs`):
- `save_layout` - Persist workspace layout
- `load_layout` - Load saved layout

### Error Handling

Return `Result<T, String>` from commands. Errors become rejected promises in TypeScript:

```rust
// Rust
Err("Error message".to_string())
```

```typescript
// TypeScript
try {
  await commandName();
} catch (error) {
  console.error(error); // "Error message"
}
```
