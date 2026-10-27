//! Prepare local models without capturing audio, creating a meeting or calling cloud AI.
use crate::{state::AppState, stt::provider::STTProvider, translation::TranslationProviderType};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Default, serde::Serialize)]
pub struct PreparationStatus {
    pub stage: String,
    pub elapsed_ms: u64,
    pub translation_ms: u64,
    pub whisper_ms: u64,
    pub error: Option<String>,
    pub cached_whisper_models: usize,
}
static STATUS: OnceLock<Mutex<PreparationStatus>> = OnceLock::new();
pub(crate) static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn publish(app: &AppHandle, status: PreparationStatus) {
    *STATUS
        .get_or_init(|| Mutex::new(PreparationStatus::default()))
        .lock()
        .unwrap() = status.clone();
    let _ = app.emit("model_preparation", status);
}
#[tauri::command]
pub fn get_model_preparation() -> PreparationStatus {
    let mut status = STATUS
        .get_or_init(|| Mutex::new(PreparationStatus::default()))
        .lock()
        .unwrap()
        .clone();
    status.cached_whisper_models = crate::stt::whisper_cpp::cached_model_count();
    status
}

#[tauri::command]
pub async fn prepare_meeting_models(
    app: AppHandle,
    you_model: Option<String>,
    them_model: Option<String>,
    you_language: Option<String>,
    them_language: Option<String>,
    translation: bool,
) -> Result<PreparationStatus, String> {
    let _gate = GATE.lock().await;
    let state = app.state::<AppState>();
    if state
        .meeting_active
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("Models are already in use by a meeting".into());
    }
    let started = std::time::Instant::now();
    let mut status = PreparationStatus {
        stage: "translation".into(),
        ..Default::default()
    };
    publish(&app, status.clone());
    let work = async {
        if translation {
            let provider = {
                state
                    .translation
                    .as_ref()
                    .ok_or("Translation is not initialized")?
                    .lock()
                    .map_err(|e| e.to_string())?
                    .get_provider()
                    .map_err(|e| e.to_string())?
            };
            if provider.provider_type() == TranslationProviderType::HyMt {
                crate::translation::hy_mt::prepare_local_translation(provider.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        status.translation_ms = started.elapsed().as_millis() as u64;
        status.stage = "whisper".into();
        publish(&app, status.clone());
        let whisper_started = std::time::Instant::now();
        for (model, language) in [(you_model, you_language), (them_model, them_language)] {
            let (Some(model), Some(language)) = (model, language) else {
                continue;
            };
            let path = {
                state
                    .model_manager
                    .as_ref()
                    .ok_or("Model manager is not initialized")?
                    .lock()
                    .map_err(|e| e.to_string())?
                    .get_model_path("whisper_cpp", &model)
                    .ok_or("Selected Whisper model is not downloaded")?
            };
            let mut provider = crate::stt::whisper_cpp::WhisperCppSTT::new(
                path.clone(),
                state.whisper_config.clone(),
            );
            provider.set_language(language.split('-').next().unwrap_or(&language));
            let (tx, _rx) = tokio::sync::mpsc::channel(16);
            provider.start_stream(tx).await.map_err(|e| e.to_string())?;
            provider.stop_stream().await.map_err(|e| e.to_string())?;
        }
        status.whisper_ms = whisper_started.elapsed().as_millis() as u64;
        Ok::<(), String>(())
    }
    .await;
    status.elapsed_ms = started.elapsed().as_millis() as u64;
    status.cached_whisper_models = crate::stt::whisper_cpp::cached_model_count();
    status.stage = if work.is_ok() { "ready" } else { "error" }.into();
    status.error = work.err();
    publish(&app, status.clone());
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    Ok(status)
}
#[tauri::command]
pub async fn release_prepared_models(app: AppHandle) -> Result<(), String> {
    let _gate = GATE
        .try_lock()
        .map_err(|_| "Local models are still preparing")?;
    let state = app.state::<AppState>();
    if state
        .meeting_active
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("End the meeting before releasing models".into());
    }
    crate::stt::whisper_cpp::release_cached_models();
    state
        .local_runtime
        .stop(crate::local_runtime::ModelRole::Translation)
        .await;
    publish(
        &app,
        PreparationStatus {
            stage: "idle".into(),
            ..Default::default()
        },
    );
    Ok(())
}
