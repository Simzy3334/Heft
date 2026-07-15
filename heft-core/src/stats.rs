//! Aggregate statistics over a completed scan.

use crate::scanner::Scan;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct BigFile {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeSlice {
    pub ext: String,
    pub bytes: u64,
    pub files: u64,
    pub frac: f64,
}

fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 8 => ext.to_ascii_lowercase(),
        _ => "(none)".to_string(),
    }
}

/// The `limit` largest files in the scan, with their relative paths.
pub fn largest_files(scan: &Scan, limit: usize) -> Vec<BigFile> {
    let mut files: Vec<(u32, u64)> = scan
        .arena
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.is_dir)
        .map(|(i, n)| (i as u32, n.size))
        .collect();
    // partial selection: only sort what we return
    let k = limit.min(files.len());
    if k == 0 {
        return Vec::new();
    }
    let pivot = k - 1;
    files.select_nth_unstable_by(pivot, |a, b| b.1.cmp(&a.1));
    files.truncate(k);
    files.sort_by(|a, b| b.1.cmp(&a.1));

    files
        .into_iter()
        .map(|(id, size)| {
            let parts = scan.path_to(id);
            let path = parts
                .iter()
                .skip(1) // drop the root segment - it's implied
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join("/");
            BigFile {
                id,
                name: scan.arena[id as usize].name.to_string(),
                path,
                size,
            }
        })
        .collect()
}

/// Bytes per file extension, largest first, everything past `limit`
/// folded into an "other" slice.
pub fn type_breakdown(scan: &Scan, limit: usize) -> Vec<TypeSlice> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    let mut total: u64 = 0;
    for node in scan.arena.iter().filter(|n| !n.is_dir) {
        let entry = map.entry(extension(&node.name)).or_insert((0, 0));
        entry.0 += node.size;
        entry.1 += 1;
        total += node.size;
    }
    let total = total.max(1);

    let mut slices: Vec<TypeSlice> = map
        .into_iter()
        .map(|(ext, (bytes, files))| TypeSlice {
            ext,
            bytes,
            files,
            frac: bytes as f64 / total as f64,
        })
        .collect();
    slices.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    if slices.len() > limit {
        let rest = slices.split_off(limit);
        let bytes: u64 = rest.iter().map(|s| s.bytes).sum();
        let files: u64 = rest.iter().map(|s| s.files).sum();
        slices.push(TypeSlice {
            ext: "other".to_string(),
            bytes,
            files,
            frac: bytes as f64 / total as f64,
        });
    }
    slices
}
