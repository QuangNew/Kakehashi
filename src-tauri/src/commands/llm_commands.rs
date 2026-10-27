use std::sync::Arc;
use tauri::{command, State};
use tokio::sync::Mutex as TokioMutex;

use crate::llm::openrouter_models;
use crate::llm::provider::LLMProvider;
use crate::llm::{create_provider, LLMRouter, ProviderConfig};
use crate::state::AppState;

pub(crate) fn pooled_config(mut config: ProviderConfig, state: &AppState) -> Result<ProviderConfig,String> {
    let kind = crate::llm::ProviderType::from_str(&config.provider_type).map_err(|e|e.to_string())?;
    if kind.requires_api_key() && config.api_key.is_none() {
        config.api_key = state.credentials.as_ref().ok_or("Credentials are not initialized")?.lock().map_err(|e|e.to_string())?.get_key(&config.provider_type)?;
    }
    if config.provider_type == "custom" && config.auth_type.as_deref().is_some_and(|s|s!="none") && config.auth_value.is_none() {
        config.auth_value = state.credentials.as_ref().ok_or("Credentials are not initialized")?.lock().map_err(|e|e.to_string())?.get_key("custom")?;
        if config.auth_value.is_none() { return Err("Select a Custom credential in API Pool".into()); }
    }
    Ok(config)
}

/// Resolve the provider for a read-only operation (model listing, connection test).
///
/// A JSON `ProviderConfig` builds a temporary provider and never touches the
/// router; a bare type name (or anything else) uses the currently active provider.
fn resolve_provider(
    llm: &std::sync::Mutex<LLMRouter>,
    provider: &str,
    state: &AppState,
) -> Result<Arc<TokioMutex<Box<dyn LLMProvider>>>, String> {
    if let Ok(config) = serde_json::from_str::<ProviderConfig>(provider) {
        let config = pooled_config(config,state)?;
        let temp =
            create_provider(&config).map_err(|e| format!("Failed to create provider: {}", e))?;
        return Ok(Arc::new(TokioMutex::new(temp)));
    }

    let router = llm
        .lock()
        .map_err(|e| format!("Failed to lock LLM router: {}", e))?;
    router
        .get_provider()
        .map_err(|e| format!("No active provider: {}", e))
}

#[command]
pub async fn set_llm_provider(
    provider: String,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Parse the provider config from the JSON string, or create a simple one from the provider name
    let config: ProviderConfig = match serde_json::from_str(&provider) {
        Ok(config) => config,
        Err(_) => {
            // Treat the input as just a provider type name
            ProviderConfig {
                provider_type: provider.clone(),
                api_key: None,
                base_url: None,
                auth_type: None,
                auth_value: None,
                auth_header: None,
            }
        }
    };
    let config = pooled_config(config,&state)?;

    let llm = state
        .llm
        .as_ref()
        .ok_or_else(|| "LLM router not initialized".to_string())?;

    let mut router = llm
        .lock()
        .map_err(|e| format!("Failed to lock LLM router: {}", e))?;

    let provider_type = config.provider_type.clone();
    if model_id
        .as_ref()
        .is_some_and(|model| model.trim().is_empty())
    {
        return Err("Choose a copilot model".into());
    }
    router
        .set_provider(config)
        .map_err(|e| format!("Failed to set provider: {}", e))?;

    router.set_active_model(model_id.unwrap_or_default());
    log::info!("Copilot provider set to: {}", provider_type);
    Ok(())
}

#[command]
pub async fn list_models(provider: String, state: State<'_, AppState>) -> Result<String, String> {
    let llm = state
        .llm
        .as_ref()
        .ok_or_else(|| "LLM router not initialized".to_string())?;

    // A JSON config uses a temporary provider; the active provider is never switched.
    let provider_arc = resolve_provider(llm, &provider, &state)?;

    let provider_guard = provider_arc.lock().await;
    let models = provider_guard
        .list_models()
        .await
        .map_err(|e| format!("Failed to list models: {}", e))?;

    serde_json::to_string(&models).map_err(|e| format!("Failed to serialize models: {}", e))
}

#[command]
pub async fn set_active_model(
    provider: String,
    model_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let llm = state
        .llm
        .as_ref()
        .ok_or_else(|| "LLM router not initialized".to_string())?;

    let mut router = llm
        .lock()
        .map_err(|e| format!("Failed to lock LLM router: {}", e))?;

    if router.active_provider_type().map(|p| p.as_str()) != Some(provider.as_str()) {
        return Err("Copilot provider changed; select the model again".into());
    }
    if model_id.trim().is_empty() {
        return Err("Choose a copilot model".into());
    }
    router.set_active_model(model_id.clone());
    log::info!("Active model set to: {}", model_id);
    Ok(())
}

#[command]
pub async fn test_llm_connection(
    provider: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let llm = state
        .llm
        .as_ref()
        .ok_or_else(|| "LLM router not initialized".to_string())?;

    // A JSON config uses a temporary provider; the active provider is never switched.
    let provider_arc = resolve_provider(llm, &provider, &state)?;

    let provider_guard = provider_arc.lock().await;
    provider_guard
        .test_connection()
        .await
        .map_err(|e| format!("Connection test failed: {}", e))
}

#[command]
pub async fn get_llm_providers() -> Result<String, String> {
    let providers = LLMRouter::get_all_providers();
    serde_json::to_string(&providers).map_err(|e| format!("Failed to serialize providers: {}", e))
}

#[command]
pub async fn list_openrouter_models(
    force_refresh: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Check cache first (unless force refresh)
    if !force_refresh {
        let cache_guard = state
            .openrouter_cache
            .lock()
            .map_err(|e| format!("Failed to lock cache: {}", e))?;
        if let Some(ref cache) = *cache_guard {
            if cache.is_valid() {
                log::info!(
                    "OpenRouter models: returning {} cached models",
                    cache.models.len()
                );
                return serde_json::to_string(&cache.models)
                    .map_err(|e| format!("Failed to serialize: {}", e));
            }
        }
    }

    // Get API key from CredentialManager
    let api_key = {
        let cred_mgr = state
            .credentials
            .as_ref()
            .ok_or_else(|| "Credential manager not initialized".to_string())?;
        let cred = cred_mgr
            .lock()
            .map_err(|e| format!("Failed to lock credential manager: {}", e))?;
        cred.get_key("openrouter")
            .map_err(|e| format!("Failed to get API key: {}", e))?
            .ok_or_else(|| {
                "OpenRouter API key not found. Please enter your API key first.".to_string()
            })?
    };

    // Fetch from API
    let models = openrouter_models::fetch_openrouter_models(&api_key).await?;
    let model_count = models.len();

    // Update cache
    {
        let mut cache_guard = state
            .openrouter_cache
            .lock()
            .map_err(|e| format!("Failed to lock cache: {}", e))?;
        *cache_guard = Some(openrouter_models::OpenRouterModelCache::new(models.clone()));
    }

    log::info!(
        "OpenRouter models: fetched and cached {} models",
        model_count
    );

    serde_json::to_string(&models).map_err(|e| format!("Failed to serialize: {}", e))
}
