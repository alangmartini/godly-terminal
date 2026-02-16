# Build Skill

Build and run Godly Terminal in development or production mode.

## Usage

```
/build [mode]
```

Where `mode` is one of:
- `dev` (default) - Start development server with hot reload
- `prod` - Build for production
- `preview` - Preview production build

## Instructions

### Development Mode (default)

Run the Tauri development server with hot reload:

```bash
cd godly-terminal && npm run tauri dev
```

This starts:
- Vite dev server for the TypeScript frontend
- Tauri in development mode with the Rust backend

### Production Build

Build the application for release:

```bash
cd godly-terminal && npm run tauri build
```

This will:
1. Type-check TypeScript (`tsc`)
2. Bundle the frontend with Vite
3. Compile the Rust backend in release mode
4. Package the application

The built executable will be in `godly-terminal/src-tauri/target/release/`.

### Preview

Preview the production frontend build (without Tauri):

```bash
cd godly-terminal && npm run preview
```

## Troubleshooting

If the build fails:

1. **TypeScript errors**: Run `cd godly-terminal && npx tsc --noEmit` to see type errors
2. **Rust errors**: Run `cd godly-terminal/src-tauri && cargo check` to see Rust errors
3. **Missing dependencies**: Run `cd godly-terminal && npm install` and `cd godly-terminal/src-tauri && cargo build`
