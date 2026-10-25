use crate::local_runtime::{LocalModelsConfig, ModelRole, RuntimeStatus};
use crate::state::AppState;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn get_local_models_config(app: AppHandle) -> LocalModelsConfig {
    app.state::<AppState>().local_runtime.config()
}

#[tauri::command]
pub async fn save_local_models_config(
    app: AppHandle,
    config: LocalModelsConfig,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state
        .meeting_active
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("End the meeting before changing local model files".into());
    }
    let previous = state.local_runtime.config();
    state.local_runtime.configure(config.clone()).await?;
    if previous.models_dir != config.models_dir
        || previous.embeddings.file != config.embeddings.file
    {
        if let Some(rag) = &state.rag {
            let manager = rag.lock().map_err(|e| e.to_string())?;
            if manager.config().embedding_provider == "local_gguf" {
                state
                    .rag_generation
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if let Some(database) = &state.database {
                    crate::rag::RagManager::clear_index(
                        database.lock().map_err(|e| e.to_string())?.connection(),
                    )?;
                }
            }
        }
    }
    if let Some(router) = &state.translation {
        router.lock().map_err(|e| e.to_string())?.clear_cache();
    }
    if let Some(manager) = &state.model_manager {
        manager
            .lock()
            .map_err(|e| e.to_string())?
            .set_models_dir(config.models_dir.into());
    }
    Ok(())
}

#[tauri::command]
pub async fn start_local_model(app: AppHandle, role: ModelRole) -> Result<String, String> {
    app.state::<AppState>().local_runtime.ensure(role).await
}

#[tauri::command]
pub async fn stop_local_model(app: AppHandle, role: ModelRole) -> Result<(), String> {
    app.state::<AppState>().local_runtime.stop(role).await;
    Ok(())
}

#[tauri::command]
pub fn get_local_models_status(app: AppHandle) -> Vec<RuntimeStatus> {
    app.state::<AppState>().local_runtime.status()
}
