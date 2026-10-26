use crate::{state::AppState, translation::TranslationResult};
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub fn begin_live_translation(app: AppHandle, meeting_id: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let current = state.live_meeting.lock().map_err(|e| e.to_string())?;
    if current.as_ref().map(|m| m.id.as_str()) != Some(meeting_id.as_str()) {
        return Err("The meeting is no longer active".into());
    }
    Ok(state.live_translations.begin(meeting_id))
}

#[tauri::command]
pub fn end_live_translation(app: AppHandle, session_id: String) {
    app.state::<AppState>().live_translations.end(&session_id);
}

#[tauri::command]
pub async fn translate_live_segment(
    app: AppHandle,
    meeting_id: String,
    session_id: String,
    segment_id: String,
    text: String,
    source_lang: String,
    target_lang: String,
    request_id: String,
) -> Result<Option<TranslationResult>, String> {
    if text.trim().is_empty() || text.len() > 16000 {
        return Err("Transcript segment is empty or too long".into());
    }
    let state = app.state::<AppState>();
    let queued_at = std::time::Instant::now();
    let ticket = state.live_translations.reserve(
        &session_id,
        &meeting_id,
        &segment_id,
        &target_lang,
        request_id.clone(),
    )?;
    let _permit = tokio::select! {
        _ = ticket.cancel.cancelled() => return Ok(None),
        permit = state.live_translations.gate.clone().acquire_owned() => permit.map_err(|e| e.to_string())?,
    };
    let started = std::time::Instant::now();
    let queue_ms = queued_at.elapsed().as_millis() as u64;
    let work = async {
        let (provider, name) = {
            let router = state
                .translation
                .as_ref()
                .ok_or("Translation is not initialized")?
                .lock()
                .map_err(|e| e.to_string())?;
            (
                router.get_provider().map_err(|e| e.to_string())?,
                router.active_provider_name(),
            )
        };
        let context = state.live_translations.context(&ticket, &source_lang, &target_lang);
        let translated = provider
            .translate_with_context(&text, Some(&source_lang), &target_lang, &context)
            .await
            .map_err(|e| e.to_string())?;
        Ok::<_, String>((translated, name))
    };
    let result = tokio::select! {
        _ = ticket.cancel.cancelled() => return Ok(None),
        result = work => result,
    };
    let (translated_text, provider) = match result {
        Ok(result) => result,
        Err(error) => {
            state.live_translations.finish(&ticket, || {});
            return Err(error);
        }
    };
    let inference_ms = started.elapsed().as_millis() as u64;
    let result = TranslationResult {
        meeting_id: Some(meeting_id.clone()),
        request_id: Some(request_id),
        segment_id: Some(segment_id.clone()),
        original_text: text,
        translated_text,
        source_lang,
        target_lang,
        provider,
    };
    let mut save_error = None;
    let current = state.live_translations.finish(&ticket, || {
        if let Some(database) = &state.database {
            match database.lock() {
                Ok(db) => {
                    if let Err(error) = crate::db::translation::save_translation(
                        db.connection(),
                        &segment_id,
                        &meeting_id,
                        &result.source_lang,
                        &result.target_lang,
                        &result.original_text,
                        &result.translated_text,
                        &result.provider,
                    ) {
                        save_error = Some(error.to_string());
                    }
                }
                Err(error) => save_error = Some(error.to_string()),
            }
        }
        if save_error.is_none() {
            let _ = app.emit("translation_result", &result);
            let _ = app.emit("translation_performance", serde_json::json!({
                "meeting_id":meeting_id, "queue_ms":queue_ms, "inference_ms":inference_ms,
            }));
        }
    });
    if let Some(error) = save_error {
        return Err(format!("Could not save translation: {}", error));
    }
    if current {
        state.live_translations.remember(&ticket, &result.source_lang, &result.target_lang,
            &result.original_text, &result.translated_text);
    }
    Ok(current.then_some(result))
}
