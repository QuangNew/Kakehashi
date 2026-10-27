pub mod audio;
pub mod commands;
pub mod context;
pub mod credentials;
pub mod data_paths;
pub mod db;
pub mod intelligence;
pub mod llm;
pub mod local_runtime;
pub mod ort_runtime;
pub mod rag;
pub mod state;
pub mod stt;
pub mod translation;
pub mod tray;

use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::{
    tray::{MouseButton, TrayIconEvent},
    Emitter, Manager,
};

// == MODULE COMMANDS: audio ==
use commands::audio_commands;
use commands::lifecycle;
use commands::claude_cli_commands;
use commands::translation_model_commands;
// == MODULE COMMANDS: stt ==
use commands::stt_commands;
// == MODULE COMMANDS: llm ==
use commands::llm_commands;
// == MODULE COMMANDS: intelligence ==
use commands::intelligence_commands;
// == MODULE COMMANDS: context ==
use commands::context_commands;
// == MODULE COMMANDS: credentials ==
use commands::credential_commands;
// == MODULE COMMANDS: meetings ==
use commands::meeting_chat_commands;
use commands::meeting_commands;
// == MODULE COMMANDS: settings ==
use commands::settings_commands;
// == MODULE COMMANDS: models ==
use commands::model_commands;
use commands::preparation_commands;
// == MODULE COMMANDS: stealth ==
use commands::stealth_commands;
// == MODULE COMMANDS: gemini cache ==
// == MODULE COMMANDS: rag ==
use commands::rag_commands;
// == MODULE COMMANDS: recording ==
use commands::recording_commands;
// == MODULE COMMANDS: translation ==
use commands::translation_commands;
// == MODULE COMMANDS: translation models ==
// == MODULE COMMANDS: tray ==
use commands::tray_commands;
// == MODULE COMMANDS: updater ==
// == MODULE COMMANDS: claude cli ==

/// Enable live blur-behind on a window via the undocumented `SetWindowCompositionAttribute`
/// (user32). `DwmEnableBlurBehindWindow` (used automatically by tao for `transparent: true`)
/// is a no-op on Windows 10/11 — this is the API that still works, and (unlike the old
/// Aero blur) composites the live desktop/other windows behind, not just the wallpaper.
/// Gradient color alpha is 0, so it adds no tint of its own; the overlay's own CSS
/// (`.overlay-bg`) provides the adjustable dark tint on top.
#[cfg(target_os = "windows")]
fn enable_live_blur_behind(hwnd_raw: *mut std::ffi::c_void) {
    use std::ffi::c_void;
    use windows::core::PCSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

    #[repr(C)]
    struct AccentPolicy {
        accent_state: u32,
        accent_flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }

    #[repr(C)]
    struct WindowCompositionAttribData {
        attrib: u32,
        pv_data: *mut c_void,
        cb_data: usize,
    }

    const WCA_ACCENT_POLICY: u32 = 19;
    const ACCENT_ENABLE_BLURBEHIND: u32 = 3;

    // Undocumented API: not present in user32.lib's import table, so it must be
    // resolved at runtime via GetProcAddress instead of statically linked.
    type SetWindowCompositionAttributeFn =
        unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> i32;

    unsafe {
        let Ok(module) = GetModuleHandleA(PCSTR(b"user32.dll\0".as_ptr())) else {
            return;
        };
        let Some(proc) = GetProcAddress(module, PCSTR(b"SetWindowCompositionAttribute\0".as_ptr()))
        else {
            return;
        };
        let set_window_composition_attribute: SetWindowCompositionAttributeFn =
            std::mem::transmute(proc);

        let mut policy = AccentPolicy {
            accent_state: ACCENT_ENABLE_BLURBEHIND,
            accent_flags: 0,
            // ABGR. Alpha must be non-zero or DWM treats blur-behind as disabled
            // and the window falls back to opaque. Alpha=1 is visually negligible.
            gradient_color: 0x01000000,
            animation_id: 0,
        };

        let mut data = WindowCompositionAttribData {
            attrib: WCA_ACCENT_POLICY,
            pv_data: &mut policy as *mut _ as *mut c_void,
            cb_data: std::mem::size_of::<AccentPolicy>(),
        };

        set_window_composition_attribute(HWND(hwnd_raw), &mut data);
    }
}

/// Show the launcher window and hide the overlay window.
fn show_launcher(app: &tauri::AppHandle) {
    if let Some(launcher) = app.get_webview_window("launcher") {
        let _ = launcher.show();
        let _ = launcher.unminimize();
        let _ = launcher.set_focus();
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
}

/// Show the overlay window and hide the launcher window.
fn show_overlay(app: &tauri::AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.show();
        let _ = overlay.set_focus();
    }
    if let Some(launcher) = app.get_webview_window("launcher") {
        let _ = launcher.hide();
    }
}

/// Launching NexQ again while it runs: bring up the overlay during a meeting
/// (hiding it would hide the meeting), else the launcher.
fn show_for_second_launch(app: &tauri::AppHandle) {
    let meeting_active = app
        .try_state::<AppState>()
        .map(|s| s.meeting_active.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false);
    if !meeting_active {
        show_launcher(app);
        return;
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.show();
        let _ = overlay.unminimize();
        let _ = overlay.set_focus();
    }
}

/// Hide all windows (minimize to tray).
fn hide_all(app: &tauri::AppHandle) {
    if let Some(launcher) = app.get_webview_window("launcher") {
        let _ = launcher.hide();
    }
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize env_logger so all log::info/warn/error macros produce output.
    // Without this, every log statement in the backend is a no-op.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Must be first: a second launch forwards to this process and exits,
        // preventing duplicate tray icons.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_for_second_launch(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let mut app_state = AppState::new();

            // -- Initialize DatabaseManager --
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");
            match db::DatabaseManager::new(app_data_dir.clone()) {
                Ok(db_mgr) => {
                    app_state.database = Some(Arc::new(Mutex::new(db_mgr)));
                    log::info!("Database initialized successfully");
                }
                Err(e) => {
                    log::error!("Failed to initialize database: {}", e);
                }
            }

            if let Ok(mut audio) = app_state.audio.lock() {
                if let Some(audio) = audio.as_mut() {
                    audio.recordings_dir = Some(app_data_dir.join("recordings"));
                }
            }
            if let Some(parent) = app_data_dir.parent() {
                let legacy = parent.join("com.nexq.app");
                // A failed migration keeps readable original paths and retries on a later launch.
                if let Some(db) = &app_state.database {
                    match data_paths::migrate_legacy_files(
                        db.lock().unwrap().connection(),
                        &app_data_dir,
                        &legacy,
                    ) {
                        Ok(count) => log::info!(
                            "Migrated {count} context/recording paths; original files retained"
                        ),
                        Err(error) => log::warn!("Legacy path migration deferred: {error}"),
                    }
                }
                if legacy.join("recordings").is_dir() {
                    let _ = app
                        .asset_protocol_scope()
                        .allow_directory(legacy.join("recordings"), true);
                }
            }

            // -- Initialize ModelManager --
            let models_dir = app_data_dir.join("models");
            let model_mgr = stt::local_engines::ModelManager::new(models_dir);
            app_state.model_manager = Some(Arc::new(Mutex::new(model_mgr)));
            match app_state
                .local_runtime
                .initialize(app_data_dir.join("local-models.json"))
            {
                Ok(true) => {
                    if let Some(manager) = &app_state.model_manager {
                        manager
                            .lock()
                            .unwrap()
                            .set_models_dir(app_state.local_runtime.config().models_dir.into());
                    }
                }
                Ok(false) => {}
                Err(error) => log::error!("Cannot load local models configuration: {}", error),
            }
            log::info!("Model manager initialized");

            // -- Initialize CredentialManager --
            let mut cred_mgr = credentials::CredentialManager::new();
            if let Err(error) = cred_mgr.initialize_pool(app_data_dir.join("api-pool.json")) {
                log::error!("Cannot initialize API Pool: {}",error);
            }
            app_state.credentials = Some(Arc::new(Mutex::new(cred_mgr)));
            log::info!("Credential manager initialized");

            // -- Initialize ContextManager --
            let ctx_mgr = context::ContextManager::new(app_data_dir.join("context"));
            app_state.context = Some(Arc::new(Mutex::new(ctx_mgr)));
            log::info!("Context manager initialized");

            // -- Initialize RagManager --
            let mut rag_config = std::fs::read(app_data_dir.join("rag-config.json"))
                .ok()
                .and_then(|data| serde_json::from_slice::<rag::config::RagConfig>(&data).ok())
                .unwrap_or_default();
            if rag_config.embedding_provider != "local_gguf" {
                rag_config.embedding_provider = "local_gguf".into();
                rag_config.embedding_model = "embeddings".into();
                rag_config.embedding_dimensions = 1024;
                if let Some(db) = &app_state.database {
                    let _ = rag::RagManager::clear_index(db.lock().unwrap().connection());
                }
                let _ = std::fs::write(
                    app_data_dir.join("rag-config.json"),
                    serde_json::to_vec_pretty(&rag_config).unwrap(),
                );
            }
            let rag_mgr = rag::RagManager::new(rag_config);
            app_state.rag = Some(Arc::new(Mutex::new(rag_mgr)));
            log::info!("RAG manager initialized");

            // -- Restore persisted context resources from DB --
            // Load stored metadata first (while holding DB lock), then restore into
            // ContextManager (different lock), then clean up any stale DB entries.
            if let (Some(db_arc), Some(ctx_arc)) = (&app_state.database, &app_state.context) {
                let stored = {
                    match db_arc.lock() {
                        Ok(db_guard) => db::context::list_context_resources(db_guard.connection())
                            .unwrap_or_default(),
                        Err(_) => Vec::new(),
                    }
                };

                let mut missing_ids: Vec<String> = Vec::new();

                if let Ok(mut ctx) = ctx_arc.lock() {
                    for db_res in stored {
                        let id = db_res.id.clone();
                        let res = context::ContextResource {
                            id: db_res.id,
                            name: db_res.name,
                            file_type: db_res.file_type,
                            file_path: db_res.file_path,
                            size_bytes: db_res.size_bytes as u64,
                            token_count: db_res.token_count as usize,
                            preview: db_res.preview,
                            loaded_at: db_res.loaded_at,
                        };
                        match ctx.restore_resource(res) {
                            Ok(()) => {}
                            Err(e) => {
                                log::warn!(
                                    "Context resource {} no longer on disk, removing: {}",
                                    id,
                                    e
                                );
                                missing_ids.push(id);
                            }
                        }
                    }
                    log::info!(
                        "Restored {} context resource(s) from DB",
                        ctx.list_resources().len()
                    );
                }

                // Clean up DB entries whose files have been deleted outside the app
                if !missing_ids.is_empty() {
                    if let Ok(db_guard) = db_arc.lock() {
                        for id in &missing_ids {
                            let _ = db::context::delete_context_resource(db_guard.connection(), id);
                            let _ = db::rag::delete_chunks_by_file(db_guard.connection(), id);
                        }
                        log::info!("Cleaned up {} stale context resource(s)", missing_ids.len());
                    }
                }
            }

            // -- Initialize STTRouter --
            let mut stt_router = stt::STTRouter::new();
            stt_router.set_app_handle(app.handle().clone());
            app_state.stt = Some(Arc::new(Mutex::new(stt_router)));
            log::info!("STT router initialized");

            // -- Initialize LLMRouter with auto-detected provider --
            let mut llm_router = llm::LLMRouter::new();

            let _ = llm_router.set_provider(llm::ProviderConfig {
                provider_type: "local_gguf".into(),
                api_key: None,
                base_url: None,
                auth_type: None,
                auth_value: None,
                auth_header: None,
            });
            llm_router.set_active_model("copilot".into());

            app_state.llm = Some(Arc::new(Mutex::new(llm_router)));
            log::info!("LLM router initialized");

            // -- Initialize IntelligenceEngine --
            let intel_engine = intelligence::IntelligenceEngine::new();
            app_state.intelligence = Some(Arc::new(Mutex::new(intel_engine)));
            log::info!("Intelligence engine initialized");

            // -- Initialize TranslationRouter --
            let mut translation_router = translation::TranslationRouter::new();
            translation_router.set_app_handle(app.handle().clone());
            translation_router.set_local_runtime(app_state.local_runtime.clone());
            app_state.translation = Some(Arc::new(Mutex::new(translation_router)));
            log::info!("Translation router initialized");

            let opus_mt_dir = app_data_dir.join("models").join("opus_mt");
            if let Some(router) = &app_state.translation {
                router.lock().unwrap().set_opus_mt_models_dir(opus_mt_dir.clone());
            }
            // -- Initialize OPUS-MT ModelManager --
            let opus_mt_mgr = translation::opus_mt_manager::OpusMtManager::new(opus_mt_dir);
            // Sync the active model ID to the translation router
            if let Some(active_id) = opus_mt_mgr.active_model_id() {
                if let Some(tr) = &app_state.translation {
                    if let Ok(mut router) = tr.lock() {
                        router.set_opus_mt_active_model(Some(active_id.to_string()));
                    }
                }
            }
            app_state.opus_mt_manager = Some(Arc::new(Mutex::new(opus_mt_mgr)));
            log::info!("OPUS-MT model manager initialized");

            // -- Claude CLI session manager (sessions start lazily) --
            app_state.agent_sessions = Arc::new(llm::cli_agent::manager::AgentSessionManager::new(
                Arc::new(llm::cli_agent::manager::ClaudeSpawner),
                Arc::new(llm::cli_agent::manager::TauriStatusSink(app.handle().clone())),
                app_state.claude_cli_config.clone(),
            )
            .with_catalog(app_state.claude_models.clone()));
            tauri::async_runtime::spawn(llm::cli_agent::manager::run_idle_reaper(
                app_state.agent_sessions.clone(),
            ));
            // Re-probe an unavailable Claude every 60 s (no model call).
            tauri::async_runtime::spawn(llm::cli_agent::routing::run_health_probe(
                app.handle().clone(),
            ));

            app.manage(app_state);

            // -- Initialize TrayManager (non-fatal — app works without enhanced tray) --
            match tray::IconSet::new(include_bytes!("../icons/kakehashi/icon.png")) {
                Ok(icon_set) => {
                    let manager = tray::TrayManager::new(icon_set);
                    let state = app.state::<AppState>();
                    *state.tray_manager.lock().unwrap() = Some(manager);

                    // Build initial idle menu (best-effort)
                    if let Ok(menu) = tray::menu::build_idle_menu(app.handle()) {
                        if let Some(tray_icon) = app.tray_by_id("main") {
                            let _ = tray_icon.set_menu(Some(menu));
                        }
                    }
                    log::info!("TrayManager initialized successfully");
                }
                Err(e) => {
                    log::error!(
                        "Failed to initialize TrayManager: {}. Tray will use defaults.",
                        e
                    );
                }
            }

            // -- Handle tray menu item clicks --
            let app_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                let id = event.id().as_ref();
                // A deferred quit is waiting on a recording; don't start new work.
                if lifecycle::quit_pending() && id != "quit" {
                    log::info!("Ignoring tray action '{}' while quit is pending", id);
                    return;
                }
                match id {
                    "start_meeting" => {
                        let _ = _app.emit("tray_start_meeting", ());
                        show_overlay(&app_handle);
                    }
                    "stop_meeting" => {
                        let _ = _app.emit("tray_stop_meeting", ());
                    }
                    "toggle_mic" => {
                        let _ = _app.emit("tray_toggle_mic", ());
                    }
                    "toggle_system" => {
                        let _ = _app.emit("tray_toggle_system", ());
                    }
                    "toggle_stealth" => {
                        let _ = _app.emit("tray_toggle_stealth", ());
                    }
                    "show_overlay" => {
                        let _ = _app.emit("tray_show_overlay", ());
                        show_overlay(&app_handle);
                    }
                    "copy_ai_answer" => {
                        let _ = _app.emit("tray_copy", "ai_answer");
                    }
                    "copy_action_items" => {
                        let _ = _app.emit("tray_copy", "action_items");
                    }
                    "copy_summary" => {
                        let _ = _app.emit("tray_copy", "summary");
                    }
                    "copy_transcript" => {
                        let _ = _app.emit("tray_copy", "transcript");
                    }
                    "settings" => {
                        let _ = _app.emit("tray_open_settings", ());
                        show_launcher(&app_handle);
                    }
                    "quit" => {
                        _app.exit(0);
                    }
                    _ => {
                        // Check for recent meeting clicks (id format: "recent_{id}")
                        if let Some(meeting_id) = id.strip_prefix("recent_") {
                            let _ = _app.emit("tray_open_meeting", meeting_id.to_string());
                            show_launcher(&app_handle);
                        }
                    }
                }
            });

            // -- Handle tray icon click events (single/double/middle) --
            // -- Handle tray icon click events --
            // Windows fires Click on EVERY mouse-up, including both ups in a double-click.
            // This makes Click unreliable for toggle. Standard Windows UX:
            //   Left double-click → show/toggle window
            //   Middle click → mute toggle
            //   Right-click → context menu (handled by Tauri menu system)
            let tray_app = app.handle().clone();
            if let Some(tray_icon) = app.tray_by_id("main") {
                tray_icon.on_tray_icon_event(move |_tray, event| {
                    match event {
                        TrayIconEvent::DoubleClick { button, .. } => match button {
                            MouseButton::Left => {
                                tray::click::handle_single_click(&tray_app);
                            }
                            _ => {}
                        },
                        TrayIconEvent::Click { button, .. } => match button {
                            MouseButton::Middle => {
                                tray::click::handle_middle_click(&tray_app);
                            }
                            _ => {} // Ignore left-click — use double-click instead
                        },
                        _ => {}
                    }
                });
            }

            // -- Intercept launcher window close: hide instead of quit --
            if let Some(launcher) = app.get_webview_window("launcher") {
                let close_app = app.handle().clone();
                launcher.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        hide_all(&close_app);
                    }
                });
            }

            // -- Enable live blur-behind on the overlay window for real desktop passthrough --
            #[cfg(target_os = "windows")]
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Ok(hwnd) = overlay.hwnd() {
                    enable_live_blur_behind(hwnd.0);
                }
            }

            log::info!("NexQ initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            audio_commands::start_capture,
            audio_commands::ensure_ipolicy_override,
            stt_commands::estimate_deepgram_cost,
            stt_commands::update_deepgram_config,
            stt_commands::update_groq_config,
            stt_commands::set_pause_threshold,
            stt_commands::get_pause_threshold,
            meeting_commands::start_past_meeting_session,
            meeting_commands::close_past_meeting_session,
            rag_commands::test_ollama_embedding_connection,
            rag_commands::pull_embedding_model,
            translation_model_commands::list_opus_mt_models,
            translation_model_commands::download_opus_mt_model,
            translation_model_commands::cancel_opus_mt_download,
            translation_model_commands::delete_opus_mt_model,
            translation_model_commands::activate_opus_mt_model,
            claude_cli_commands::claude_cli_status,
            claude_cli_commands::set_claude_cli_config,
            claude_cli_commands::claude_cli_models,
            claude_cli_commands::seed_claude_cli_models,
            claude_cli_commands::get_agent_sessions,
            claude_cli_commands::open_claude_login_terminal,

            // == COMMANDS: audio ==
            audio_commands::list_audio_devices,
            audio_commands::stop_capture,
            audio_commands::get_audio_level,
            audio_commands::test_audio_device,
            audio_commands::start_audio_test,
            audio_commands::stop_audio_test,
            audio_commands::set_recording_enabled,
            audio_commands::get_audio_sessions,
            audio_commands::get_audio_peak_levels,
            audio_commands::start_capture_per_party,
            audio_commands::start_device_monitor,
            audio_commands::stop_device_monitor,
            audio_commands::set_source_muted,
            audio_commands::get_mute_status,
            // == COMMANDS: stt ==
            stt_commands::set_stt_provider,
            stt_commands::set_stt_language,
            commands::local_model_commands::get_local_models_config,
            commands::local_model_commands::save_local_models_config,
            commands::local_model_commands::start_local_model,
            commands::local_model_commands::stop_local_model,
            commands::local_model_commands::get_local_models_status,
            commands::live_translation_commands::begin_live_translation,
            commands::live_translation_commands::end_live_translation,
            commands::live_translation_commands::translate_live_segment,
            stt_commands::test_stt_connection,
            stt_commands::get_available_stt_providers,
            stt_commands::update_whisper_dual_pass_config,
            // == COMMANDS: llm ==
            llm_commands::set_llm_provider,
            llm_commands::list_models,
            llm_commands::set_active_model,
            llm_commands::test_llm_connection,
            llm_commands::get_llm_providers,
            llm_commands::list_openrouter_models,
            // == COMMANDS: intelligence ==
            intelligence_commands::generate_assist,
            intelligence_commands::cancel_generation,
            intelligence_commands::set_auto_trigger,
            intelligence_commands::set_context_window_seconds,
            intelligence_commands::push_transcript,
            intelligence_commands::update_action_configs,
            intelligence_commands::get_action_configs,
            intelligence_commands::set_active_scenario,
            intelligence_commands::update_speaker_context,
            // == COMMANDS: context ==
            context_commands::load_context_file,
            context_commands::remove_context_file,
            context_commands::list_context_resources,
            context_commands::set_custom_instructions,
            context_commands::get_assembled_context,
            context_commands::get_token_budget,
            // == COMMANDS: credentials ==
            credential_commands::store_api_key,
            credential_commands::get_api_key,
            credential_commands::delete_api_key,
            credential_commands::has_api_key,
            credential_commands::get_api_pool,
            credential_commands::test_api_pool_entry,
            credential_commands::save_api_pool_entry,
            credential_commands::delete_api_pool_entry,
            credential_commands::bind_api_pool_entry,
            // == COMMANDS: meetings ==
            meeting_commands::start_meeting,
            meeting_commands::end_meeting,
            meeting_commands::list_meetings,
            meeting_commands::get_meeting,
            meeting_commands::delete_meeting,
            meeting_commands::search_meetings,
            meeting_commands::append_transcript_segment,
            meeting_commands::save_meeting_ai_interactions,
            meeting_commands::rename_meeting,
            meeting_commands::update_meeting_summary,
            meeting_commands::save_meeting_speakers,
            meeting_commands::save_meeting_bookmarks,
            meeting_commands::add_meeting_bookmark,
            meeting_commands::update_meeting_bookmark,
            meeting_commands::delete_meeting_bookmark,
            meeting_commands::save_meeting_action_items,
            meeting_commands::update_action_item,
            meeting_commands::delete_action_item,
            meeting_commands::save_meeting_topic_sections,
            meeting_commands::rename_speaker,
            meeting_commands::update_meeting_mode,
            meeting_chat_commands::ask_meeting,
            meeting_chat_commands::list_meeting_chat,
            // == COMMANDS: settings ==
            settings_commands::get_config,
            settings_commands::set_config,
            // == COMMANDS: models ==
            model_commands::list_local_stt_engines,
            model_commands::download_local_stt_model,
            model_commands::get_model_downloads,
            preparation_commands::prepare_meeting_models,
            preparation_commands::get_model_preparation,
            preparation_commands::release_prepared_models,
            model_commands::cancel_model_download,
            model_commands::delete_local_stt_model,
            // == COMMANDS: stealth ==
            stealth_commands::set_stealth_mode,
            // == COMMANDS: tray ==
            tray_commands::set_tray_state,
            tray_commands::set_tray_tooltip,
            tray_commands::set_meeting_start_time,
            tray_commands::rebuild_tray_menu,
            // == COMMANDS: gemini cache ==
            // == COMMANDS: rag ==
            rag_commands::rebuild_rag_index,
            rag_commands::rebuild_file_index,
            rag_commands::clear_rag_index,
            rag_commands::get_rag_status,
            rag_commands::test_rag_search,
            rag_commands::get_rag_config,
            rag_commands::update_rag_config,
            rag_commands::test_embedding_connection,
            rag_commands::test_rag_answer,
            rag_commands::remove_file_rag_index,
            // == COMMANDS: recording ==
            recording_commands::get_recording_info,
            recording_commands::get_recording_file_url,
            recording_commands::delete_recording,
            // == COMMANDS: translation ==
            translation_commands::set_translation_provider,
            translation_commands::list_translation_models,
            translation_commands::translate_text,
            translation_commands::translate_segments,
            translation_commands::translate_batch,
            translation_commands::detect_language,
            translation_commands::test_translation_connection,
            translation_commands::get_translation_languages,
            translation_commands::get_meeting_translations,
            translation_commands::get_all_meeting_translations,
            translation_commands::export_translated_transcript,
            translation_commands::set_translation_languages,
            // == COMMANDS: translation models ==
            // == COMMANDS: updater ==
            lifecycle::is_quit_pending,
            // == COMMANDS: claude cli ==
        ])
        .build(tauri::generate_context!())
        .expect("error while building NexQ")
        .run(|app, event| {
            // Tray Quit (`app.exit(0)`) and any other exit request: install a
            // downloaded update first (exits inside the call on success), or
            // defer the quit while the last recording is still saving.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = &event {
                if *code != Some(tauri::RESTART_EXIT_CODE) {
                    lifecycle::on_exit_requested(app, api);
                }
            }
            // Stop only model servers owned by this application.
            if let tauri::RunEvent::Exit = &event {
                tauri::async_runtime::block_on(app.state::<AppState>().local_runtime.shutdown());
                let sessions = app.state::<AppState>().agent_sessions.clone();
                tauri::async_runtime::block_on(sessions.close_all());
            }
        });
}
