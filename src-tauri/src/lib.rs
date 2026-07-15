//! Heft's Tauri layer: a thin bridge between heft-core and the webview.
//!
//! Design rules that keep this layer efficient:
//! * The scan arena lives in Rust behind a mutex - it is NEVER serialised
//!   wholesale to the frontend. A 100k-node tree crossing IPC as JSON would
//!   dwarf the scan itself.
//! * The frontend asks narrow questions ("treemap of node 42 at 1200x800",
//!   "top 30 files") and gets small, ready-to-render answers.
//! * Scanning runs on a plain OS thread; progress reaches the UI as events
//!   throttled at the source (~12 Hz), so a million-file scan emits a few
//!   dozen events, not a million.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    scan: Mutex<Option<heft_core::Scan>>,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    files: u64,
    bytes: u64,
}

#[derive(Clone, Serialize)]
struct DonePayload {
    root_path: String,
    files: u64,
    bytes: u64,
    skipped: u64,
    nodes: usize,
}

#[derive(Serialize)]
struct Crumb {
    id: u32,
    name: String,
}

// ------------------------------------------------------------------ commands

#[tauri::command]
fn start_scan(app: AppHandle, path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    std::thread::spawn(move || {
        let result = heft_core::scan(&target, |files, bytes| {
            let _ = app.emit("scan-progress", ProgressPayload { files, bytes });
        });
        match result {
            Ok(scan) => {
                let payload = DonePayload {
                    root_path: scan.root_path.to_string_lossy().to_string(),
                    files: scan.files,
                    bytes: scan.bytes,
                    skipped: scan.skipped,
                    nodes: scan.arena.len(),
                };
                let state: State<AppState> = app.state();
                *state.scan.lock().unwrap() = Some(scan);
                let _ = app.emit("scan-done", payload);
            }
            Err(err) => {
                let _ = app.emit("scan-error", err.to_string());
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn treemap(
    state: State<AppState>,
    node: u32,
    width: f64,
    height: f64,
) -> Result<Vec<heft_core::Rect>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    if (node as usize) >= scan.arena.len() {
        return Err("unknown node".into());
    }
    // cull anything smaller than ~24px^2 - invisible and wasteful to ship
    Ok(heft_core::layout(scan, node, width, height, 24.0))
}

#[tauri::command]
fn breadcrumb(state: State<AppState>, node: u32) -> Result<Vec<Crumb>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    if (node as usize) >= scan.arena.len() {
        return Err("unknown node".into());
    }
    Ok(scan
        .path_to(node)
        .into_iter()
        .map(|(id, name)| Crumb { id, name })
        .collect())
}

#[tauri::command]
fn largest_files(state: State<AppState>, limit: usize) -> Result<Vec<heft_core::BigFile>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    Ok(heft_core::largest_files(scan, limit.min(200)))
}

#[tauri::command]
fn type_breakdown(state: State<AppState>, limit: usize) -> Result<Vec<heft_core::TypeSlice>, String> {
    let guard = state.scan.lock().unwrap();
    let scan = guard.as_ref().ok_or("no scan loaded")?;
    Ok(heft_core::type_breakdown(scan, limit.min(30)))
}

// ---------------------------------------------------------------------- app

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { scan: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            start_scan,
            treemap,
            breadcrumb,
            largest_files,
            type_breakdown
        ])
        .run(tauri::generate_context!())
        .expect("error while running Heft");
}
