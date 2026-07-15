//! cargo run --release --example bench -- /path/to/scan
use std::time::Instant;
fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "/usr".into());
    let t0 = Instant::now();
    let scan = heft_core::scan(std::path::Path::new(&path), |_, _| {}).unwrap();
    let dt = t0.elapsed().as_secs_f64();
    println!(
        "{path}: {} files, {:.2} GB, {} nodes in {:.2}s ({:.0} files/s)",
        scan.files, scan.bytes as f64 / 1e9, scan.arena.len(), dt, scan.files as f64 / dt
    );
    let t1 = Instant::now();
    let rects = heft_core::layout(&scan, 0, 1600.0, 900.0, 24.0);
    println!("treemap: {} rects in {:.2}ms", rects.len(), t1.elapsed().as_secs_f64() * 1000.0);
}
