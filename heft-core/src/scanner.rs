//! Parallel filesystem scanner.
//!
//! The tree is an *arena*: one flat `Vec<Node>` where children point at their
//! parent by `u32` index. Compared to a pointer-based tree this is dramatically
//! more cache-friendly, needs no per-node allocation beyond the name, and lets
//! the UI address any node by a plain integer across the IPC boundary.
//!
//! Parallelism: the root's immediate subdirectories are scanned on scoped OS
//! threads (`std::thread::scope`, no runtime, no dependencies). Each worker
//! builds a private sub-arena with zero contention; sub-arenas are then merged
//! by offsetting child indices - an O(nodes) splice. Progress is shared
//! through two relaxed atomics that the caller may sample from any thread.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

pub const NO_PARENT: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Box<str>,
    pub size: u64,
    pub is_dir: bool,
    pub parent: u32,
    pub children: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub bytes: AtomicU64,
    pub skipped: AtomicU64,
}

#[derive(Debug)]
pub struct Scan {
    pub arena: Vec<Node>,
    pub root_path: PathBuf,
    pub files: u64,
    pub bytes: u64,
    pub skipped: u64,
}

impl Scan {
    pub fn root(&self) -> &Node {
        &self.arena[0]
    }

    /// Breadcrumb from root to `id` as (id, name) pairs.
    pub fn path_to(&self, id: u32) -> Vec<(u32, String)> {
        let mut out = Vec::new();
        let mut cur = id;
        loop {
            let node = &self.arena[cur as usize];
            out.push((cur, node.name.to_string()));
            if node.parent == NO_PARENT {
                break;
            }
            cur = node.parent;
        }
        out.reverse();
        out
    }
}

/// Scan one directory level into `arena`, recursing depth-first.
/// Returns the total size of the subtree rooted at `node_idx`.
fn scan_into(arena: &mut Vec<Node>, node_idx: u32, dir: &Path, progress: &Progress) -> u64 {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => {
            progress.skipped.fetch_add(1, Ordering::Relaxed);
            return 0;
        }
    };

    let mut total: u64 = 0;
    for entry in entries {
        let Ok(entry) = entry else {
            progress.skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            progress.skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        // symlinks are recorded as zero-cost leaves: following them risks
        // cycles and double-counting; sizing them misattributes the target
        if file_type.is_symlink() {
            continue;
        }
        let name: Box<str> = entry.file_name().to_string_lossy().into();

        if file_type.is_dir() {
            let child_idx = arena.len() as u32;
            arena.push(Node {
                name,
                size: 0,
                is_dir: true,
                parent: node_idx,
                children: Vec::new(),
            });
            arena[node_idx as usize].children.push(child_idx);
            let sub = scan_into(arena, child_idx, &entry.path(), progress);
            arena[child_idx as usize].size = sub;
            total += sub;
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let child_idx = arena.len() as u32;
            arena.push(Node {
                name,
                size,
                is_dir: false,
                parent: node_idx,
                children: Vec::new(),
            });
            arena[node_idx as usize].children.push(child_idx);
            total += size;
            progress.files.fetch_add(1, Ordering::Relaxed);
            progress.bytes.fetch_add(size, Ordering::Relaxed);
        }
    }
    total
}

/// Splice a worker's private sub-arena into the shared one.
/// Every index in `sub` shifts by `offset`; the sub-root re-parents to `parent`.
fn merge_arena(arena: &mut Vec<Node>, mut sub: Vec<Node>, parent: u32) -> u32 {
    let offset = arena.len() as u32;
    for node in sub.iter_mut() {
        if node.parent == NO_PARENT {
            node.parent = parent;
        } else {
            node.parent += offset;
        }
        for child in node.children.iter_mut() {
            *child += offset;
        }
    }
    arena.extend(sub);
    offset
}

/// Scan `root_path`, parallelising across its immediate subdirectories.
///
/// `on_progress` is invoked from the coordinating thread roughly every 80 ms
/// with (files, bytes) - throttle-free for the caller, cheap for the scanner.
pub fn scan<F: FnMut(u64, u64)>(root_path: &Path, mut on_progress: F) -> std::io::Result<Scan> {
    let meta = fs::metadata(root_path)?;
    if !meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path is not a directory",
        ));
    }

    let progress = Progress::default();
    let root_name: Box<str> = root_path
        .file_name()
        .map(|n| n.to_string_lossy().into())
        .unwrap_or_else(|| root_path.to_string_lossy().into());

    let mut arena: Vec<Node> = vec![Node {
        name: root_name,
        size: 0,
        is_dir: true,
        parent: NO_PARENT,
        children: Vec::new(),
    }];

    // Partition the root's entries: files stay on this thread, each
    // subdirectory becomes a parallel work item.
    let mut subdirs: Vec<(Box<str>, PathBuf)> = Vec::new();
    let mut root_total: u64 = 0;
    for entry in fs::read_dir(root_path)? {
        let Ok(entry) = entry else {
            progress.skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            progress.skipped.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let name: Box<str> = entry.file_name().to_string_lossy().into();
        if file_type.is_dir() {
            subdirs.push((name, entry.path()));
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let idx = arena.len() as u32;
            arena.push(Node {
                name,
                size,
                is_dir: false,
                parent: 0,
                children: Vec::new(),
            });
            arena[0].children.push(idx);
            root_total += size;
            progress.files.fetch_add(1, Ordering::Relaxed);
            progress.bytes.fetch_add(size, Ordering::Relaxed);
        }
    }

    // Scan subtrees in parallel; each worker owns a private arena.
    let worker_results: Vec<(Box<str>, Vec<Node>, u64)> = thread::scope(|scope| {
        let handles: Vec<_> = subdirs
            .into_iter()
            .map(|(name, path)| {
                let progress = &progress;
                scope.spawn(move || {
                    let mut sub = vec![Node {
                        name: name.clone(),
                        size: 0,
                        is_dir: true,
                        parent: NO_PARENT,
                        children: Vec::new(),
                    }];
                    let size = scan_into(&mut sub, 0, &path, progress);
                    sub[0].size = size;
                    (name, sub, size)
                })
            })
            .collect();

        // coordinating thread doubles as the progress pump
        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            loop {
                if handle.is_finished() {
                    results.push(handle.join().expect("scanner worker panicked"));
                    break;
                }
                on_progress(
                    progress.files.load(Ordering::Relaxed),
                    progress.bytes.load(Ordering::Relaxed),
                );
                thread::sleep(std::time::Duration::from_millis(80));
            }
        }
        results
    });

    for (_, sub, size) in worker_results {
        let sub_root = merge_arena(&mut arena, sub, 0);
        arena[0].children.push(sub_root);
        root_total += size;
    }
    arena[0].size = root_total;

    // deterministic ordering: children sorted by size desc everywhere -
    // the treemap, largest-files and UI all rely on this invariant
    let sizes: Vec<u64> = arena.iter().map(|n| n.size).collect();
    for node in arena.iter_mut() {
        node.children
            .sort_by(|a, b| sizes[*b as usize].cmp(&sizes[*a as usize]));
    }

    Ok(Scan {
        arena,
        root_path: root_path.to_path_buf(),
        files: progress.files.load(Ordering::Relaxed),
        bytes: progress.bytes.load(Ordering::Relaxed),
        skipped: progress.skipped.load(Ordering::Relaxed),
    })
}
