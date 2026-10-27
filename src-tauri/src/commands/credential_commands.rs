use tauri::{command, State};

use crate::credentials::pool::{SaveEntry, Snapshot};
use crate::state::AppState;
use tauri::{AppHandle, Emitter};

fn pool_changed(app: &AppHandle, state: &AppState) {
    // Rebuild the selected clients with current bindings; never keep using a
    // deleted credential merely because a router cached its old key.
    let config = state
        .llm
        .as_ref()
        .and_then(|router| router.lock().ok()?.active_config());
    if let Some(mut config) = config {
        if crate::credentials::pool::route(&config.provider_type).is_some() {
            config.api_key = None;
            config.auth_value = None;
            let updated = crate::commands::llm_commands::pooled_config(config, state);
            if let Some(router) = &state.llm {
                if let Ok(mut router) = router.lock() {
                    let applied = updated
                        .and_then(|config| router.set_provider(config).map_err(|e| e.to_string()));
                    if applied.is_err() {
                        router.invalidate_credentials();
                    }
                }
            }
        }
    }
    if let (Some(credentials), Some(router)) = (&state.credentials, &state.translation) {
        if let (Ok(credentials), Ok(mut router)) = (credentials.lock(), router.lock()) {
            use crate::translation::TranslationProviderType as Kind;
            if let Some(kind) = router.active_type().cloned() {
                let name = match kind {
                    Kind::Groq => Some("groq"),
                    Kind::Gemini => Some("gemini"),
                    Kind::Microsoft => Some("microsoft"),
                    Kind::Google => Some("google"),
                    Kind::Deepl => Some("deepl"),
                    _ => None,
                };
                if let Some(name) = name {
                    let key = credentials.get_key(&format!("translation_{name}"));
                    let result = match key {
                        Ok(Some(key)) => match kind {
                            Kind::Groq | Kind::Gemini => {
                                let model =
                                    router.active_api_model().unwrap_or_default().to_string();
                                router.set_api_provider(kind, model, key)
                            }
                            _ => {
                                match kind {
                                    Kind::Microsoft => router.set_microsoft_credentials(
                                        key,
                                        credentials
                                            .get_key("translation_microsoft_region")
                                            .ok()
                                            .flatten(),
                                    ),
                                    Kind::Google => router.set_google_credentials(key),
                                    Kind::Deepl => router.set_deepl_credentials(key),
                                    _ => (),
                                }
                                router.set_provider(kind)
                            }
                        },
                        _ => Err(crate::translation::TranslationError::NotConfigured(
                            "Select an API Pool key".into(),
                        )),
                    };
                    if result.is_err() {
                        router.invalidate_credentials();
                    }
                }
            }
        }
    }
    if let Ok(mut cache) = state.openrouter_cache.lock() {
        *cache = None;
    }
    let _ = app.emit("api_pool_changed", ());
}

#[command]
pub fn get_api_pool(state: State<'_, AppState>) -> Result<Snapshot, String> {
    state
        .credentials
        .as_ref()
        .ok_or("Credentials are not initialized")?
        .lock()
        .map_err(|e| e.to_string())?
        .pool_snapshot()
}

#[command]
pub async fn test_api_pool_entry(id: String, state: State<'_, AppState>) -> Result<usize, String> {
    let (entry, key) = {
        let credentials = state
            .credentials
            .as_ref()
            .ok_or("Credentials are not initialized")?
            .lock()
            .map_err(|e| e.to_string())?;
        let entry = credentials
            .pool_snapshot()?
            .entries
            .into_iter()
            .find(|e| e.entry.id == id)
            .ok_or("API Pool entry no longer exists")?;
        let key = credentials
            .get_key(&format!("api_pool:{id}"))?
            .ok_or("API key is missing")?;
        (entry.entry, key)
    };
    if !["groq", "gemini", "openai", "anthropic", "openrouter"].contains(&entry.provider.as_str()) {
        return Err("Choose a provider-specific connection test in its settings".into());
    }
    let config = crate::llm::ProviderConfig {
        provider_type: entry.provider,
        api_key: Some(key),
        base_url: None,
        auth_type: None,
        auth_value: None,
        auth_header: None,
    };
    let provider = crate::llm::create_provider(&config)?;
    provider.list_models().await.map(|models|models.len()).map_err(|_|"The provider did not return a model catalog. Check this key's permissions, connection and quota in API Pool.".into())
}
#[command]
pub fn save_api_pool_entry(
    app: AppHandle,
    input: SaveEntry,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id = state
        .credentials
        .as_ref()
        .ok_or("Credentials are not initialized")?
        .lock()
        .map_err(|e| e.to_string())?
        .pool_save(input)?;
    pool_changed(&app, &state);
    Ok(id)
}
#[command]
pub fn delete_api_pool_entry(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .credentials
        .as_ref()
        .ok_or("Credentials are not initialized")?
        .lock()
        .map_err(|e| e.to_string())?
        .pool_delete(&id)?;
    pool_changed(&app, &state);
    Ok(())
}
#[command]
pub fn bind_api_pool_entry(
    app: AppHandle,
    usage: String,
    provider: String,
    id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .credentials
        .as_ref()
        .ok_or("Credentials are not initialized")?
        .lock()
        .map_err(|e| e.to_string())?
        .pool_bind(&usage, &provider, id)?;
    pool_changed(&app, &state);
    Ok(())
}

#[command]
pub async fn store_api_key(
    app: AppHandle,
    provider: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cred_mgr = state
        .credentials
        .as_ref()
        .ok_or_else(|| "Credential manager not initialized".to_string())?;

    let cred = cred_mgr
        .lock()
        .map_err(|e| format!("Failed to lock credential manager: {}", e))?;

    cred.store_key(&provider, &key)?;
    drop(cred);
    pool_changed(&app, &state);
    Ok(())
}

#[command]
pub async fn get_api_key(
    provider: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let cred_mgr = state
        .credentials
        .as_ref()
        .ok_or_else(|| "Credential manager not initialized".to_string())?;

    let cred = cred_mgr
        .lock()
        .map_err(|e| format!("Failed to lock credential manager: {}", e))?;

    cred.get_key(&provider)
}

#[command]
pub async fn delete_api_key(
    app: AppHandle,
    provider: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cred_mgr = state
        .credentials
        .as_ref()
        .ok_or_else(|| "Credential manager not initialized".to_string())?;

    let cred = cred_mgr
        .lock()
        .map_err(|e| format!("Failed to lock credential manager: {}", e))?;

    cred.delete_key(&provider)?;
    drop(cred);
    pool_changed(&app, &state);
    Ok(())
}

#[command]
pub async fn has_api_key(provider: String, state: State<'_, AppState>) -> Result<bool, String> {
    let cred_mgr = state
        .credentials
        .as_ref()
        .ok_or_else(|| "Credential manager not initialized".to_string())?;

    let cred = cred_mgr
        .lock()
        .map_err(|e| format!("Failed to lock credential manager: {}", e))?;

    cred.has_key(&provider)
}
