//! heft-core: everything Heft does that isn't a window.
//!
//! Kept free of Tauri so it compiles anywhere, tests in milliseconds, and
//! could power a CLI or a different GUI unchanged.

pub mod scanner;
pub mod stats;
pub mod treemap;

pub use scanner::{scan, Node, Scan, NO_PARENT};
pub use stats::{largest_files, type_breakdown, BigFile, TypeSlice};
pub use treemap::{layout, Rect};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Build a deterministic fixture tree:
    /// root/
    ///   big.bin      4096 B
    ///   docs/
    ///     a.txt       100 B
    ///     b.txt       300 B
    ///   media/
    ///     clip.mp4   2000 B
    ///     art.png     500 B
    ///     nested/
    ///       deep.log   50 B
    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("heft_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::create_dir_all(dir.join("media/nested")).unwrap();
        fs::write(dir.join("big.bin"), vec![0u8; 4096]).unwrap();
        fs::write(dir.join("docs/a.txt"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("docs/b.txt"), vec![0u8; 300]).unwrap();
        fs::write(dir.join("media/clip.mp4"), vec![0u8; 2000]).unwrap();
        fs::write(dir.join("media/art.png"), vec![0u8; 500]).unwrap();
        fs::write(dir.join("media/nested/deep.log"), vec![0u8; 50]).unwrap();
        dir
    }

    #[test]
    fn scan_counts_every_byte_once() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        assert_eq!(scan.bytes, 4096 + 100 + 300 + 2000 + 500 + 50);
        assert_eq!(scan.files, 6);
        assert_eq!(scan.root().size, scan.bytes);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn directory_sizes_roll_up() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let find = |name: &str| {
            scan.arena
                .iter()
                .find(|n| &*n.name == name)
                .unwrap_or_else(|| panic!("missing node {name}"))
        };
        assert_eq!(find("docs").size, 400);
        assert_eq!(find("media").size, 2550);
        assert_eq!(find("nested").size, 50);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn children_sorted_descending_and_parents_consistent() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        for (idx, node) in scan.arena.iter().enumerate() {
            let mut prev = u64::MAX;
            for &child in &node.children {
                let child_node = &scan.arena[child as usize];
                assert_eq!(child_node.parent, idx as u32, "parent link broken");
                assert!(child_node.size <= prev, "children not sorted desc");
                prev = child_node.size;
            }
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn breadcrumb_walks_to_root() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let deep = scan
            .arena
            .iter()
            .position(|n| &*n.name == "deep.log")
            .unwrap() as u32;
        let names: Vec<String> = scan.path_to(deep).into_iter().map(|(_, n)| n).collect();
        assert_eq!(&names[1..], &["media", "nested", "deep.log"]);
        let _ = fs::remove_dir_all(dir);
    }

    // ------------------------------------------------------------- treemap
    #[test]
    fn treemap_areas_proportional_and_in_bounds() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let rects = layout(&scan, 0, 800.0, 600.0, 0.0);
        let viewport = 800.0 * 600.0;
        let eps = 1e-6;
        let mut covered = 0.0;
        for rect in &rects {
            assert!(rect.x >= -eps && rect.y >= -eps, "rect out of bounds");
            assert!(rect.x + rect.w <= 800.0 + eps && rect.y + rect.h <= 600.0 + eps);
            let expected = rect.size as f64 / scan.root().size as f64 * viewport;
            assert!(
                (rect.w * rect.h - expected).abs() < 1.0,
                "area not proportional: {} vs {}",
                rect.w * rect.h,
                expected
            );
            covered += rect.w * rect.h;
        }
        assert!((covered - viewport).abs() < 1.0, "rects must tile the viewport");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn treemap_rects_do_not_overlap() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let rects = layout(&scan, 0, 640.0, 480.0, 0.0);
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let sep = a.x + a.w <= b.x + 1e-6
                    || b.x + b.w <= a.x + 1e-6
                    || a.y + a.h <= b.y + 1e-6
                    || b.y + b.h <= a.y + 1e-6;
                assert!(sep, "rects {} and {} overlap", a.name, b.name);
            }
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn treemap_aggregates_tiny_rects() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        // huge min_px forces everything but the biggest into the tail
        let rects = layout(&scan, 0, 100.0, 100.0, 3000.0);
        assert!(rects.iter().any(|r| r.id == u32::MAX), "expected aggregate rect");
        let total: u64 = rects.iter().map(|r| r.size).sum();
        assert_eq!(total, scan.root().size, "aggregation must conserve bytes");
        let _ = fs::remove_dir_all(dir);
    }

    // --------------------------------------------------------------- stats
    #[test]
    fn largest_files_ordered_with_paths() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let top = largest_files(&scan, 3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].name, "big.bin");
        assert_eq!(top[1].name, "clip.mp4");
        assert_eq!(top[1].path, "media/clip.mp4");
        assert!(top[0].size >= top[1].size && top[1].size >= top[2].size);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn type_breakdown_sums_to_total_and_folds_tail() {
        let dir = fixture();
        let scan = scan(&dir, |_, _| {}).unwrap();
        let slices = type_breakdown(&scan, 2);
        let total: u64 = slices.iter().map(|s| s.bytes).sum();
        assert_eq!(total, scan.bytes);
        assert_eq!(slices[0].ext, "bin");
        assert_eq!(slices.last().unwrap().ext, "other");
        let _ = fs::remove_dir_all(dir);
    }
}
