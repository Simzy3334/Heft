# Heft

A fast, cross-platform disk space visualizer: pick a folder, Heft scans it in
parallel and renders a zoomable squarified treemap, plus a sidebar of the
largest files and a by-type size breakdown. Built with React (frontend), Rust
(scanning/layout logic), and Tauri (native shell/IPC).

## Architecture

Three layers, each with a narrow job:

```
React (canvas treemap, hover/zoom)          src/            ~50 KB gzipped, no UI framework
        │  narrow queries: "treemap of node 42 @ 1200x800"
        ▼
Tauri commands (thin glue)                  src-tauri/      arena NEVER crosses IPC wholesale
        ▼
heft-core (pure Rust, zero-dep)             heft-core/      scanner · squarify · stats
```

- **`heft-core`** has no Tauri dependency, compiles standalone, and is where
  all real logic and all tests live. Treat it as the source of truth; the
  other two layers should stay thin.
- **The arena**: the scanned tree is one flat `Vec<Node>` with `u32`
  parent/child indices (`heft-core/src/scanner.rs`) — not `Rc<RefCell<>>`.
  Any node is addressable across IPC as a plain integer.
- **IPC discipline**: the scan result never leaves Rust wholesale. The
  frontend asks narrow questions (`treemap(node, width, height)`,
  `largest_files(limit)`) and gets small, ready-to-render answers back
  (`src-tauri/src/lib.rs`). Sub-pixel rects are aggregated into a single
  "N small items" cell **in Rust** before they ever touch JSON.
- **Parallel scanning**: root subdirectories scan on scoped OS threads
  (`std::thread::scope`, no rayon/tokio). Each worker fills a private
  sub-arena; sub-arenas merge by index offsetting. Progress flows through
  two relaxed atomics, throttled to ~12 Hz before reaching the UI as events.
- **Squarified treemap** (Bruls et al. 2000), implemented from scratch in
  `heft-core/src/treemap.rs` and property-tested: areas proportional to
  bytes, rectangles never overlap, layout tiles the viewport exactly.

## Project structure

```
heft-core/src/
  scanner.rs     parallel scan → arena tree
  treemap.rs     squarified layout + small-rect aggregation
  stats.rs       largest files (partial selection), type breakdown
src-tauri/src/
  lib.rs         5 commands + 3 events, scan state behind a Mutex
src/
  App.tsx        phase machine: idle → scanning → ready
  components/
    Treemap.tsx  canvas renderer: DPR-aware, hover, zoom, labels
    Sidebar.tsx  largest files + type bars
  lib/types.ts   shared contracts + mineral color palette
```

## Commands

```bash
npm install                 # frontend deps
npm run dev                  # vite dev server only
npm run tauri dev            # full app, hot reload
npm run build                 # tsc (strict) + vite build
npm run tauri build          # native installer

cargo test -p heft-core --manifest-path heft-core/Cargo.toml   # 9 unit/property tests
cargo check --manifest-path src-tauri/Cargo.toml                # compile the Tauri layer
cargo run --release --example bench --manifest-path heft-core/Cargo.toml -- ~/some/folder
```

CI (`.github/workflows/ci.yml`) runs these three independently: `heft-core`
tests, the strict frontend build, and a Tauri `cargo check`.

## Conventions worth preserving

- New scanning/layout/stats logic goes in `heft-core`, with unit or property
  tests alongside it (see `heft-core/src/lib.rs` `#[cfg(test)]` module) — not
  behind a Tauri command where it can't be tested without a webview.
- Don't add a new Tauri command that returns the whole arena or a large
  subtree; return the smallest projection the frontend actually needs, same
  pattern as `treemap`/`largest_files`/`type_breakdown`.
- Frontend stays dependency-light: no chart library, no CSS framework. The
  canvas renderer and the mineral palette (`src/lib/types.ts`) are the whole
  visual system — copper/media, sand/images, moss/code, slate/docs,
  leather/archives, lichen/data, gunmetal/binaries. No purple gradients.
- `heft-core` has one dependency (`serde`, for IPC serialization). Keep it
  that way — the zero-dep-beyond-serde property is load-bearing for the
  "compiles anywhere, tests in milliseconds" claim in the README.
