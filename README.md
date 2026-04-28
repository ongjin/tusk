# Tusk

> Postgres, with intelligence.

AI-native Postgres client built with Tauri 2 + React + Rust. The roadmap and
design rationale live in [`PLAN.md`](./PLAN.md).

## Status

Week 1 scaffold. Real Postgres connection, SQL editor, and AI features land in
the following weeks per the plan.

## Development

```bash
# Install dependencies (uses pnpm via corepack)
pnpm install

# Run the desktop app in dev mode
pnpm tauri dev

# Frontend-only dev server (no Rust)
pnpm dev
```

## Quality gates

```bash
pnpm typecheck        # tsc --noEmit
pnpm lint             # eslint
pnpm format:check     # prettier --check
pnpm rust:check       # cargo check
pnpm rust:lint        # cargo clippy -D warnings
pnpm rust:fmt:check   # cargo fmt --check
```

`pnpm format` and `pnpm lint:fix` apply auto-fixes. `pnpm rust:fmt` formats
Rust sources.

## Project structure

```
src/
  components/ui/   shadcn/ui primitives
  features/        feature modules (added per roadmap)
  hooks/           cross-cutting React hooks
  lib/             utils + Tauri invoke wrappers
  store/           zustand stores
  types/           shared TS types

src-tauri/
  src/commands/    Tauri command handlers
  src/lib.rs       Tauri builder entry
```

## License

MIT — see [`LICENSE`](./LICENSE).
