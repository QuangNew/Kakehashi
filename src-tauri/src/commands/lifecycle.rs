//! Normal desktop exit waits for post-meeting recording writes.
use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, ExitRequestApi, Manager};

static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn quit_pending() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn is_quit_pending() -> bool {
    quit_pending()
}

pub fn on_exit_requested(app: &AppHandle, api: &ExitRequestApi) {
    let state = app.state::<AppState>();
    if state.recording_jobs.load(Ordering::SeqCst) == 0 {
        return;
    }
    api.prevent_exit();
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    for window in app.webview_windows().values() {
        let _ = window.hide();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while app
            .state::<AppState>()
            .recording_jobs
            .load(Ordering::SeqCst)
            > 0
        {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        app.exit(0);
    });
}
