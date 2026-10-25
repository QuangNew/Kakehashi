//! Replay a local 16 kHz mono WAV through the real STT provider at capture speed.
//! No microphone, database, user settings or external AI API is used.
use nexq_lib::{
    audio::{AudioChunk, AudioSource},
    stt::{
        provider::{DualPassConfig, STTProvider},
        whisper_cpp::WhisperCppSTT,
    },
    translation::{hy_mt::HyMtTranslator, TranslationProvider},
};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let wav = args.get(1).ok_or_else(|| {
        anyhow::anyhow!("usage: stt_realtime_bench WAV [seconds] [--translate] [--raw]")
    })?;
    let limit: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(90.0);
    let include_text = args.iter().any(|s| s == "--text");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let mut reader = hound::WavReader::open(wav)?;
    anyhow::ensure!(
        reader.spec().sample_rate == 16000
            && reader.spec().channels == 1
            && reader.spec().bits_per_sample == 16,
        "Expected 16 kHz mono PCM16 WAV"
    );
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .take((limit * 16000.0) as usize)
        .collect::<Result<_, _>>()?;
    let model = root.join("models/whisper_cpp/ggml-small.bin");
    if args.iter().any(|s| s == "--raw") {
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};
        let (options, backend, device) = nexq_lib::stt::whisper_cpp::context_parameters();
        println!(
            "{}",
            serde_json::json!({"kind":"backend", "backend":backend, "device":device})
        );
        let ctx = WhisperContext::new_with_params(model.to_str().unwrap(), options)?;
        let mut state = ctx.create_state()?;
        let window: f32 = args
            .iter()
            .find_map(|s| s.strip_prefix("--window=").and_then(|s| s.parse().ok()))
            .unwrap_or(3.0);
        anyhow::ensure!(
            (0.3..=30.0).contains(&window),
            "Raw window must be between 0.3 and 30 seconds"
        );
        for (index, chunk) in samples.chunks((window * 16000.0) as usize).enumerate() {
            let input: Vec<f32> = chunk.iter().map(|s| *s as f32 / 32768.0).collect();
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("ja"));
            params.set_n_threads(4);
            params.set_no_context(true);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            let start = Instant::now();
            state.full(params, &input)?;
            let mut text = String::new();
            if include_text {
                for segment in state.as_iter() {
                    text.push_str(&segment.to_str_lossy()?);
                }
            }
            println!(
                "{}",
                serde_json::json!({"kind":"raw", "index":index, "audio_ms":chunk.len()/16, "inference_ms":start.elapsed().as_millis(), "text":include_text.then_some(text)})
            );
        }
        return Ok(());
    }
    let translate = args.iter().any(|s| s == "--translate");
    let runtime = nexq_lib::local_runtime::shared();
    let translator = HyMtTranslator::new(runtime.clone());
    let translation_loading = Instant::now();
    if translate {
        runtime
            .initialize(root.join(".cache/local-model-smoke/local-models.json"))
            .map_err(anyhow::Error::msg)?;
        nexq_lib::translation::hy_mt::prepare_local_translation(&translator).await?;
        println!(
            "{}",
            serde_json::json!({"kind":"translation_ready", "elapsed_ms":translation_loading.elapsed().as_millis()})
        );
        runtime
            .ensure(nexq_lib::local_runtime::ModelRole::Translation)
            .await
            .map_err(anyhow::Error::msg)?;
    }
    let mut provider = WhisperCppSTT::new(model, Arc::new(RwLock::new(DualPassConfig::default())));
    provider.set_language("ja");
    let (tx, mut rx) = mpsc::channel(256);
    let session_start = Instant::now();
    provider
        .start_stream(tx.clone())
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    // Match the desktop: two loaded Whisper contexts, with a quiet VI microphone.
    let mut microphone = if args.iter().any(|s| s == "--dual") {
        let mut mic = WhisperCppSTT::new(
            root.join("models/whisper_cpp/ggml-small.bin"),
            Arc::new(RwLock::new(DualPassConfig::default())),
        );
        mic.set_language("vi");
        mic.start_stream(tx.clone())
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Some(mic)
    } else {
        None
    };
    drop(tx);
    println!(
        "{}",
        serde_json::json!({"kind":"ready", "elapsed_ms":session_start.elapsed().as_millis(), "audio_ms":samples.len()/16})
    );
    let start = Instant::now();
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    provider.set_time_origin(epoch);
    if let Some(mic) = &mut microphone {
        mic.set_time_origin(epoch);
    }
    let (translation_tx, mut translation_rx) =
        mpsc::channel::<(String, Option<u64>, String, Instant)>(256);
    let translation_task = tokio::spawn(async move {
        let mut translated = 0;
        while let Some((text, audio_end, segment, queued)) = translation_rx.recv().await {
            let called = Instant::now();
            match translator.translate(&text, Some("ja"), "vi").await {
                Ok(result) => {
                    translated += 1;
                    println!(
                        "{}",
                        serde_json::json!({"kind":"translation", "segment":segment, "chars":result.chars().count(), "text":include_text.then_some(&result), "elapsed_ms":start.elapsed().as_millis(), "inference_ms":called.elapsed().as_millis(), "queue_ms":called.duration_since(queued).as_millis(), "lag_ms":audio_end.map(|end| (start.elapsed().as_millis() as u64).saturating_sub(end))})
                    );
                }
                Err(error) => println!(
                    "{}",
                    serde_json::json!({"kind":"translation_error", "error":error.to_string()})
                ),
            }
        }
        translated
    });
    let receiver = tokio::spawn(async move {
        let (mut finals, mut interims) = (0, 0);
        while let Some(result) = rx.recv().await {
            if result.is_final {
                finals += 1;
            } else {
                interims += 1;
            }
            let end = result.duration_ms.map(|d| result.timestamp_ms + d);
            println!(
                "{}",
                serde_json::json!({"kind":"transcript", "final":result.is_final, "segment":result.segment_id, "chars":result.text.chars().count(), "text":include_text.then_some(&result.text), "elapsed_ms":start.elapsed().as_millis(), "audio_start_ms":result.timestamp_ms, "duration_ms":result.duration_ms, "lag_ms":end.map(|end| (start.elapsed().as_millis() as u64).saturating_sub(end))})
            );
            if translate && result.is_final {
                // Match the live hook's 200 ms correction debounce.
                tokio::time::sleep(Duration::from_millis(200)).await;
                let _ = translation_tx
                    .send((
                        result.text,
                        end,
                        result.segment_id.unwrap_or_default(),
                        Instant::now(),
                    ))
                    .await;
            }
        }
        (finals, interims)
    });
    let mut fed = 0;
    for chunk in samples.chunks(1600) {
        fed += chunk.len();
        tokio::time::sleep_until(tokio::time::Instant::from_std(
            start + Duration::from_secs_f64(fed as f64 / 16000.0),
        ))
        .await;
        provider
            .feed_audio(AudioChunk {
                pcm_data: chunk.to_vec(),
                source: AudioSource::System,
                timestamp_ms: epoch + fed as u64 / 16,
                is_speech: true,
            })
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if let Some(mic) = &mut microphone {
            mic.feed_audio(AudioChunk {
                pcm_data: vec![0; chunk.len()],
                source: AudioSource::Mic,
                timestamp_ms: epoch + fed as u64 / 16,
                is_speech: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        if fed % 16000 == 0 {
            println!(
                "{}",
                serde_json::json!({"kind":"performance", "elapsed_ms":start.elapsed().as_millis(), "stt":provider.performance()})
            );
        }
    }
    // A real pause flushes the last partial utterance. Keep the clock paced.
    for index in 0..10 {
        tokio::time::sleep_until(tokio::time::Instant::from_std(
            start + Duration::from_secs_f64(fed as f64 / 16000.0 + (index + 1) as f64 * 0.1),
        ))
        .await;
        provider
            .feed_audio(AudioChunk {
                pcm_data: vec![0; 1600],
                source: AudioSource::System,
                timestamp_ms: epoch + fed as u64 / 16 + (index + 1) * 100,
                is_speech: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    tokio::time::sleep(Duration::from_secs(10)).await;
    provider
        .stop_stream()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if let Some(mic) = &mut microphone {
        mic.stop_stream()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    println!(
        "{}",
        serde_json::json!({"kind":"performance_final", "stt":provider.performance()})
    );
    drop(provider);
    drop(microphone);
    let summary = tokio::time::timeout(Duration::from_secs(15), receiver).await;
    let translated = tokio::time::timeout(Duration::from_secs(30), translation_task).await;
    println!(
        "{}",
        serde_json::json!({"kind":"summary", "audio_ms":fed/16, "elapsed_ms":start.elapsed().as_millis(), "results":format!("{summary:?}"), "translated":format!("{translated:?}")})
    );
    runtime.shutdown().await;
    Ok(())
}
