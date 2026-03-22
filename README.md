# Dev Space Cleaner

A developer-focused desktop cleanup tool built with `Tauri 2 + Rust + React`.

## Implemented in V1

- Scan cleanup candidates:
  - `node_modules` in code repositories
  - `npm / pnpm / yarn` cache directories
  - Docker cleanup targets (selectable)
- Sort by:
  - size
  - last used time
- Advanced options:
  - custom scan paths (default full-disk scan)
  - ignore patterns
  - Docker cleanup scope multi-select
- Cleanup flow:
  - manual multi-select
  - confirmation prompt with risk warning
  - live cleanup logs
  - post-cleanup report
- Bilingual UI:
  - Chinese / English switch

## Development

### Prerequisites

- Node.js 18+
- pnpm 9+
- Rust toolchain (`cargo`, `rustc`) for Tauri desktop build
- Tauri platform prerequisites: [https://tauri.app/start/prerequisites/](https://tauri.app/start/prerequisites/)

### Install

```bash
pnpm install
```

### Frontend build check

```bash
pnpm build
```

### Run desktop app (requires Rust installed)

```bash
pnpm tauri dev
```

## Key Files

- Plan document: `docs/desktop-cleaner-plan.md`
- Frontend page: `src/App.tsx`
- Frontend styles: `src/App.css`
- Rust commands: `src-tauri/src/lib.rs`

## Notes

- Deleting `node_modules` requires reinstalling dependencies in those projects.
- Docker cleanup commands require Docker CLI to be installed and available in PATH.
- Full-disk scan may be heavy on large machines; use custom scan paths in Advanced options for faster results.
