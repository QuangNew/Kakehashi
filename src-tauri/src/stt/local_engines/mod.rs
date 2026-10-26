pub mod downloader;
pub mod model_discovery;
pub mod model_registry;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use model_registry::{get_engines, get_model, get_models_for_engine, ModelDefinition};

/// Manages local STT model downloads, verification, and storage.
///
/// Storage layout: `{app_data_dir}/models/{engine}/{filename}`
/// For archive models, `filename` is a directory after extraction.
pub struct ModelManager {
    models_dir: PathBuf,
    active_downloads: HashMap<String, Arc<DownloadJob>>,
}

struct DownloadJob {
    cancel: Arc<AtomicBool>,
    progress: Mutex<downloader::DownloadProgress>,
}
impl DownloadJob {
    fn snapshot(&self) -> downloader::DownloadProgress { self.progress.lock().unwrap().clone() }
    fn active(&self) -> bool {
        !matches!(self.snapshot().status.as_str(), "complete" | "error" | "cancelled")
    }
    fn report(&self, app: &AppHandle, bytes: u64, total: u64, status: &str, error: Option<String>) {
        let progress = {
            let mut p = self.progress.lock().unwrap();
            p.downloaded_bytes = bytes;
            if total > 0 { p.total_bytes = total; }
            p.percent = if p.total_bytes > 0 { (bytes as f64 / p.total_bytes as f64 * 100.0).clamp(0.0, 100.0) as f32 } else { 0.0 };
            if status == "complete" { p.percent = 100.0; }
            p.status = status.into();
            p.error = error;
            p.revision = downloader::next_revision();
            p.clone()
        };
        let _ = app.emit("model_download_progress", progress);
    }
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&models_dir) {
            log::error!("Failed to create models directory: {}", e);
        }
        Self {
            models_dir,
            active_downloads: HashMap::new(),
        }
    }

    /// Get the base models directory path.
    pub fn models_dir(&self) -> &std::path::Path {
        &self.models_dir
    }

    pub fn set_models_dir(&mut self, path: PathBuf) { self.models_dir = path; }

    /// Get the path where a model file/directory would be stored.
    fn model_file_path(&self, engine: &str, filename: &str) -> PathBuf {
        self.models_dir.join(engine).join(filename)
    }

    /// Check if a model is downloaded and exists on disk.
    pub fn is_model_downloaded(&self, engine: &str, model_id: &str) -> bool {
        if let Some(def) = get_model(engine, model_id) {
            let path = self.model_file_path(engine, def.filename);
            if def.is_archive {
                // Archive models are extracted to a directory
                path.is_dir()
            } else {
                path.exists()
            }
        } else {
            false
        }
    }

    /// Get the path to a downloaded model file/directory, if it exists.
    pub fn get_model_path(&self, engine: &str, model_id: &str) -> Option<PathBuf> {
        let def = get_model(engine, model_id)?;
        let path = self.model_file_path(engine, def.filename);
        if def.is_archive {
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        } else {
            if path.exists() {
                Some(path)
            } else {
                None
            }
        }
    }

    /// Authoritative snapshots survive tab changes and event delivery gaps.
    pub fn downloads(&self) -> Vec<downloader::DownloadProgress> {
        self.active_downloads.values().map(|job| job.snapshot()).collect()
    }

    pub fn download_model(&mut self, engine: &str, model_id: &str, app: AppHandle) -> Result<(), String> {
        let def = get_model(engine, model_id)
            .ok_or_else(|| format!("Unknown model: {engine}:{model_id}"))?;
        let key = format!("{engine}:{model_id}");
        if self.active_downloads.get(&key).is_some_and(|job| job.active()) {
            return Err("Download already in progress".into());
        }
        let installed = self.is_model_downloaded(engine, model_id);
        let engine_dir = self.models_dir.join(engine);
        std::fs::create_dir_all(&engine_dir).map_err(|e| e.to_string())?;
        let dest = if def.is_archive { engine_dir.join(format!("{}.tar.bz2", def.filename)) }
                   else { engine_dir.join(def.filename) };
        let job = Arc::new(DownloadJob {
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Mutex::new(downloader::DownloadProgress {
                engine: engine.into(), model_id: model_id.into(),
                downloaded_bytes: 0, total_bytes: def.size_bytes, percent: 0.0,
                status: "queued".into(), error: None,
                revision: downloader::next_revision(),
            }),
        });
        self.active_downloads.insert(key, job.clone());
        if installed {
            job.report(&app, def.size_bytes, def.size_bytes, "complete", None);
            return Ok(());
        }
        let definition = def.clone();
        tokio::spawn(async move {
            let result = async {
                downloader::download_file(
                    definition.download_url, &dest, definition.sha256, job.cancel.clone(),
                    |bytes, total, status| job.report(&app, bytes, total, status, None),
                ).await?;
                if definition.is_archive {
                    let current = job.snapshot();
                    job.report(&app, current.downloaded_bytes, current.total_bytes, "extracting", None);
                    let archive = dest.clone();
                    let old_raw = engine_dir.join(definition.filename);
                    if old_raw.is_file() { tokio::fs::remove_file(old_raw).await.map_err(|e| e.to_string())?; }
                    tokio::task::spawn_blocking(move || downloader::extract_tar_bz2(&archive, &engine_dir))
                        .await.map_err(|e| e.to_string())??;
                    tokio::fs::remove_file(&dest).await.map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(())
            }.await;
            let current = job.snapshot();
            match result {
                Ok(()) => job.report(&app, current.total_bytes, current.total_bytes, "complete", None),
                Err(error) => {
                    let cancelled = job.cancel.load(Ordering::Acquire);
                    let status = if cancelled { "cancelled" } else { "error" };
                    if !cancelled { log::error!("Model download failed ({}:{}): {}", definition.engine, definition.model_id, error); }
                    let bytes = tokio::fs::metadata(dest.with_extension("download")).await
                        .map(|m| m.len()).unwrap_or(current.downloaded_bytes);
                    job.report(&app, bytes, current.total_bytes, status, (!cancelled).then_some(error));
                }
            }
        });
        Ok(())
    }

    pub fn cancel_download(&mut self, engine: &str, model_id: &str) {
        if let Some(job) = self.active_downloads.get(&format!("{engine}:{model_id}")) {
            // Keep ownership until the worker exits; a second writer must not start yet.
            job.cancel.store(true, Ordering::Release);
        }
    }

    /// Delete a downloaded model from disk (handles both files and directories).
    pub fn delete_model(&mut self, engine: &str, model_id: &str) -> Result<(), String> {
        if self.active_downloads.get(&format!("{engine}:{model_id}")).is_some_and(|job| job.active()) {
            return Err("Cancel the active download before deleting this model".into());
        }
        let def = get_model(engine, model_id)
            .ok_or_else(|| format!("Unknown model: {}:{}", engine, model_id))?;

        let path = self.model_file_path(engine, def.filename);
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("Failed to delete model directory: {}", e))?;
        } else if path.is_file() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete model file: {}", e))?;
        }
        log::info!(
            "Deleted model: {}:{} from {}",
            engine,
            model_id,
            path.display()
        );
        Ok(())
    }

    /// List all engines with their models and download status.
    pub fn list_engines_with_status(&self) -> Vec<EngineWithStatus> {
        get_engines()
            .into_iter()
            .map(|eng| {
                let models = get_models_for_engine(eng.engine)
                    .iter()
                    .map(|m| ModelWithStatus {
                        definition: m.clone(),
                        is_downloaded: self.is_model_downloaded(m.engine, m.model_id),
                    })
                    .collect();
                EngineWithStatus {
                    engine: eng.engine.to_string(),
                    name: eng.name.to_string(),
                    description: eng.description.to_string(),
                    models,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelWithStatus {
    pub definition: ModelDefinition,
    pub is_downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineWithStatus {
    pub engine: String,
    pub name: String,
    pub description: String,
    pub models: Vec<ModelWithStatus>,
}
