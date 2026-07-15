# ▪ HEFT — a fast disk space visualizer

[![CI](https://github.com/Simzy3334/Heft/actions/workflows/ci.yml/badge.svg)](https://github.com/Simzy3334/Heft/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**React · Rust · Tauri.** Pick a folder; Heft weighs every byte in parallel and lays it out as a zoomable squarified treemap — copper for media, moss for code, sand for images — with the largest files and a type breakdown alongside. The scanning core clocks **~100k files in ~4 seconds on a single CPU core**.

```
┌────────────────────────────────────────────────────────────────────────┐
│ HEFT▪   /home/sim/projects        142,318 files · 61.4 GB   [RESCAN]  │
├──────────────────────────────────────────────────────┬─────────────────┤
│ projects / meetily / target                          │  BY TYPE        │
│ ┌────────────────────┬──────────┬──────────────────┐ │  ▪ .rlib ▮▮▮▮▮  │
│ │                    │          │   node_modules   │ │  ▪ .mp4  ▮▮▮    │
│ │   debug (12.4 GB)  │  release │      2.1 GB      │ │  ▪ .png  ▮▮     │
│ │                    │  4.2 GB  ├──────┬───────────┤ │                 │
│ │                    │          │ .git │ 214 small │ │  LARGEST FILES  │
│ └────────────────────┴──────────┴──────┴───────────┘ │  01 libfoo.rlib │
└──────────────────────────────────────────────────────┴─────────────────┘
```

## Architecture — and why it's fast

```
React (canvas treemap, hover/zoom)          ~50 KB gzipped, no UI framework
        │  narrow queries: "treemap of node 42 @ 1200x800"
        ▼
Tauri commands (thin glue, src-tauri/)      arena NEVER crosses IPC wholesale
        ▼
heft-core (pure Rust, zero-dep)             scanner · squarify · stats
```

**The arena.** The file tree is one flat `Vec<Node>` with `u32` parent/child indices — cache-friendly, one allocation per name and nothing else, and any node is addressable across IPC as a plain integer. A 100k-file tree costs ~10 MB, not a pointer-chasing heap salad.

**Parallel scanning, no runtime.** The root's subdirectories are scanned on scoped OS threads (`std::thread::scope` — no rayon, no tokio). Each worker builds a *private* sub-arena with zero lock contention; sub-arenas merge by index offsetting in O(nodes). Progress flows through two relaxed atomics.

**IPC discipline.** The scan result stays in Rust. The frontend asks narrow questions and receives small, ready-to-render answers. Sub-pixel rectangles are aggregated **in Rust** into a single "n small items" cell, so the IPC payload and canvas work stay bounded no matter how many files a directory holds.

**Squarified treemap** (Bruls et al. 2000) implemented from scratch and property-tested: areas proportional to bytes, rectangles never overlap, the layout tiles the viewport exactly, aggregation conserves every byte.

**Frontend restraint.** One `<canvas>`, DPR-aware for crisp hairlines, binary-search label truncation, hover hit-testing, click-to-zoom with breadcrumbs. React for state; no chart library, no CSS framework.

## Measured (single-core sandbox)

| Operation | Result |
|---|---|
| Scan `/usr` — 101,363 files / 5.43 GB / 114,181 nodes | **4.27 s** (23.7k files/s) |
| Treemap layout @ 1600×900 | **0.01 ms** |
| Frontend bundle | **50 KB** gzipped |
| `heft-core` unit tests | **9/9 passing** |

## Design

Warm graphite + amber. Monospace throughout, tabular numerals, hairline borders, sharp corners, uppercase micro-labels — an instrument, not a dashboard. File types map to a mineral palette (copper / sand / moss / slate / leather); directories render as recessed panels you can enter. Not a purple gradient in sight.

## Running it

Prereqs: [Rust](https://rustup.rs) (1.77+), Node 18+, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS (on Ubuntu: `libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev`).

```bash
git clone https://github.com/Simzy3334/Heft.git && cd Heft
npm install

npm run tauri dev      # develop with hot reload
npm run tauri build    # produce a native installer for your OS
```

The core is verifiable without any GUI toolchain:

```bash
cargo test -p heft-core --manifest-path heft-core/Cargo.toml       # 9 tests
cargo run --release --example bench --manifest-path heft-core/Cargo.toml -- ~/some/folder
```

## Project structure

```
heft/
├── heft-core/               # pure Rust, zero deps beyond serde
│   └── src/
│       ├── scanner.rs       # parallel scan → arena tree
│       ├── treemap.rs       # squarified layout + small-rect aggregation
│       └── stats.rs         # largest files (partial selection), type slices
├── src-tauri/               # thin Tauri glue
│   ├── src/lib.rs           # 5 commands + 3 events, state behind a mutex
│   ├── tauri.conf.json      # window, bundling, CSP
│   └── capabilities/        # Tauri v2 permission grants (core + dialog)
├── src/                     # React + TypeScript (strict)
│   ├── App.tsx              # phase machine: idle → scanning → ready
│   ├── components/
│   │   ├── Treemap.tsx      # canvas renderer: DPR, hover, zoom, labels
│   │   └── Sidebar.tsx      # largest files + type bars
│   └── lib/types.ts         # shared contracts + mineral palette
└── .github/workflows/ci.yml # core tests · frontend build · cargo check
```

## Design decisions worth asking me about

- **Why an arena instead of `Rc<RefCell<Node>>`?** Locality, one contiguous allocation, `u32` handles that serialize for free, and trivially safe parallel merging. Idiomatic Rust for trees isn't pointers — it's indices.
- **Why aggregate small rects in Rust rather than let the frontend cull?** Payload discipline. A `node_modules` with 40k entries becomes ~60 rects + one aggregate before it ever touches JSON.
- **Why per-subtree private arenas instead of one shared locked arena?** A shared `Mutex<Vec<Node>>` serializes the hot path. Private arenas make workers embarrassingly parallel; the merge is a cheap sequential splice.
- **Why `std::thread::scope` over rayon/tokio?** Scanning is syscall-bound, the parallelism is coarse (one thread per top-level dir), and scoped threads borrow the progress atomics directly. Zero dependencies beats a work-stealing runtime the job can't use.
- **Verification honesty:** `heft-core` (all the logic) is fully unit-tested and benchmarked; the frontend compiles under strict TypeScript and builds. The Tauri layer is deliberately thin, standard v2 APIs — compile it on your machine with `npm run tauri dev`.

## Roadmap

- [ ] Duplicate finder (size prefilter → BLAKE3 content hash)
- [ ] Delete-to-trash from the treemap with size reclaim preview
- [ ] Scan snapshots + diff view ("what grew since last week?")
- [ ] Windows drive overview (all volumes at a glance)

## License

MIT
