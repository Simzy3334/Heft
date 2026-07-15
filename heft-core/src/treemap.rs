//! Squarified treemap layout (Bruls, Huizing & van Wijk, 2000).
//!
//! Given a node's children (already sorted by size, descending - a scanner
//! invariant) and a viewport, lay them out as rectangles whose areas are
//! proportional to their sizes, greedily keeping aspect ratios close to 1
//! so labels stay readable and small files stay visible.
//!
//! Rectangles whose on-screen area would fall below `min_px` are aggregated
//! into a single trailing "…" rect instead of being drawn as sub-pixel noise -
//! this keeps the rect count (and therefore IPC payload + canvas work) bounded
//! no matter how many files a directory holds.

use crate::scanner::Scan;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Rect {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    /// lowercase file extension, "" for directories / extension-less files,
    /// "\u{2026}" for the aggregate rect
    pub ext: String,
    /// share of the parent, 0..=1
    pub frac: f64,
}

fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 8 => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Worst aspect ratio a row of areas would have at row thickness `length`.
fn worst_ratio(row: &[f64], length: f64) -> f64 {
    let sum: f64 = row.iter().sum();
    if sum <= 0.0 || length <= 0.0 {
        return f64::INFINITY;
    }
    let thickness = sum / length;
    row.iter().fold(0.0_f64, |acc, &area| {
        let other = area / thickness;
        acc.max((thickness / other).max(other / thickness))
    })
}

struct Frame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Lay out `areas` (descending) into `frame`; returns one (x,y,w,h) per area.
fn squarify(areas: &[f64], frame: &Frame) -> Vec<(f64, f64, f64, f64)> {
    let mut out = Vec::with_capacity(areas.len());
    let (mut x, mut y, mut w, mut h) = (frame.x, frame.y, frame.w, frame.h);
    let mut i = 0;

    while i < areas.len() {
        // grow the current row while it improves the worst aspect ratio
        let length = w.min(h);
        let mut row_end = i + 1;
        let mut best = worst_ratio(&areas[i..row_end], length);
        while row_end < areas.len() {
            let candidate = worst_ratio(&areas[i..row_end + 1], length);
            if candidate <= best {
                best = candidate;
                row_end += 1;
            } else {
                break;
            }
        }

        // place the row along the shorter side
        let row_sum: f64 = areas[i..row_end].iter().sum();
        if w >= h {
            // vertical strip on the left
            let strip_w = if h > 0.0 { row_sum / h } else { 0.0 };
            let mut cy = y;
            for &area in &areas[i..row_end] {
                let cell_h = if strip_w > 0.0 { area / strip_w } else { 0.0 };
                out.push((x, cy, strip_w, cell_h));
                cy += cell_h;
            }
            x += strip_w;
            w -= strip_w;
        } else {
            // horizontal strip on top
            let strip_h = if w > 0.0 { row_sum / w } else { 0.0 };
            let mut cx = x;
            for &area in &areas[i..row_end] {
                let cell_w = if strip_h > 0.0 { area / strip_h } else { 0.0 };
                out.push((cx, y, cell_w, strip_h));
                cx += cell_w;
            }
            y += strip_h;
            h -= strip_h;
        }
        i = row_end;
    }
    out
}

/// Treemap of `node_id`'s children inside a `width` x `height` viewport.
pub fn layout(scan: &Scan, node_id: u32, width: f64, height: f64, min_px: f64) -> Vec<Rect> {
    let node = &scan.arena[node_id as usize];
    let total: u64 = node.size.max(1);
    let viewport_area = width * height;
    if viewport_area <= 0.0 || node.children.is_empty() {
        return Vec::new();
    }

    // children are pre-sorted desc; split into drawn + aggregated tail
    let mut drawn: Vec<(u32, f64)> = Vec::new(); // (child id, pixel area)
    let mut tail_size: u64 = 0;
    let mut tail_count: u64 = 0;
    for &child_id in &node.children {
        let child = &scan.arena[child_id as usize];
        let px = child.size as f64 / total as f64 * viewport_area;
        if px >= min_px && child.size > 0 {
            drawn.push((child_id, px));
        } else {
            tail_size += child.size;
            tail_count += 1;
        }
    }
    let tail_px = tail_size as f64 / total as f64 * viewport_area;
    if tail_px > 0.0 {
        drawn.push((u32::MAX, tail_px)); // sentinel id for the aggregate
    }
    if drawn.is_empty() {
        return Vec::new();
    }

    let areas: Vec<f64> = drawn.iter().map(|(_, a)| *a).collect();
    let cells = squarify(&areas, &Frame { x: 0.0, y: 0.0, w: width, h: height });

    drawn
        .iter()
        .zip(cells)
        .map(|(&(id, _), (x, y, w, h))| {
            if id == u32::MAX {
                Rect {
                    id: u32::MAX,
                    x, y, w, h,
                    name: format!("{tail_count} small items"),
                    size: tail_size,
                    is_dir: false,
                    ext: "\u{2026}".to_string(),
                    frac: tail_size as f64 / total as f64,
                }
            } else {
                let child = &scan.arena[id as usize];
                Rect {
                    id,
                    x, y, w, h,
                    name: child.name.to_string(),
                    size: child.size,
                    is_dir: child.is_dir,
                    ext: if child.is_dir { String::new() } else { extension(&child.name) },
                    frac: child.size as f64 / total as f64,
                }
            }
        })
        .collect()
}
