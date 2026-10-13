// No console window behind the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use imaginify_core::{Job, Outcome, Progress, Settings};
use serde::Serialize;
use tauri::ipc::{Channel, Response};
use tauri::{Manager, State};

/// Longest side of the preview image sent to the window.
const PREVIEW_MAX_SIDE: u32 = 1600;

#[derive(Default)]
struct Batch {
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
}

/// Clears `running` however the batch ends.
struct RunningGuard<'a>(&'a AtomicBool);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
struct FileEntry {
    path: String,
    name: String,
}

#[tauri::command]
async fn scan_paths(paths: Vec<PathBuf>) -> Result<Vec<FileEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        imaginify_core::scan(&paths)
            .into_iter()
            .filter_map(|p| {
                let path = p.to_str()?.to_owned();
                let name = p.file_name()?.to_str()?.to_owned();
                Some(FileEntry { path, name })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Binary reply: width u32 LE, height u32 LE, 1 byte (0 = JPEG, 1 = PNG), then
/// the encoded preview. Raw bytes avoid a base64 round trip for large previews.
/// Previews decode one at a time, and a request that is no longer the newest
/// when its turn comes is skipped, so scrolling through a folder of large
/// photos cannot pile up decodes and memory.
#[derive(Default)]
struct PreviewGate {
    latest: AtomicU64,
    lock: Mutex<()>,
}

#[tauri::command]
async fn preview(path: PathBuf, gate: State<'_, Arc<PreviewGate>>) -> Result<Response, String> {
    let gate = Arc::clone(&gate);
    let id = gate.latest.fetch_add(1, Ordering::SeqCst) + 1;
    let p = tauri::async_runtime::spawn_blocking(move || {
        let _turn = gate.lock.lock().unwrap_or_else(|p| p.into_inner());
        if gate.latest.load(Ordering::SeqCst) != id {
            return Err("superseded by a newer preview".to_string());
        }
        imaginify_core::preview(&path, PREVIEW_MAX_SIDE).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut out = Vec::with_capacity(p.data.len() + 9);
    out.extend_from_slice(&p.width.to_le_bytes());
    out.extend_from_slice(&p.height.to_le_bytes());
    out.push(u8::from(p.mime == "image/png"));
    out.extend_from_slice(&p.data);
    Ok(Response::new(out))
}

#[tauri::command]
async fn process(
    jobs: Vec<Job>,
    settings: Settings,
    out_dir: PathBuf,
    on_progress: Channel<Progress>,
    batch: State<'_, Batch>,
) -> Result<Vec<Outcome>, String> {
    if out_dir.as_os_str().is_empty() {
        return Err("Choose an output folder first.".into());
    }
    if !out_dir.is_absolute() {
        return Err("The output folder must be a full path. Use Browse… to pick one.".into());
    }
    if batch.running.swap(true, Ordering::SeqCst) {
        return Err("A batch is already running.".into());
    }
    let _running = RunningGuard(&batch.running);
    batch.cancel.store(false, Ordering::SeqCst);
    let cancel = batch.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imaginify_core::process_batch(&jobs, &settings, &out_dir, &cancel, |p| {
            // A closed window drops the channel; the batch still finishes.
            let _ = on_progress.send(p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn cancel_processing(batch: State<'_, Batch>) {
    batch.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn default_output_dir(app: tauri::AppHandle) -> String {
    let base = app
        .path()
        .picture_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_default();
    base.join("Imaginify").to_string_lossy().into_owned()
}

#[tauri::command]
fn open_folder(path: PathBuf) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Batch::default())
        .manage(Arc::new(PreviewGate::default()))
        .invoke_handler(tauri::generate_handler![
            scan_paths,
            preview,
            process,
            cancel_processing,
            default_output_dir,
            open_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running Imaginify");
}
