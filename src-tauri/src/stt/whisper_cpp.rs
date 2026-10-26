//! Local Whisper: one inference per bounded utterance, in capture order.
//! Continuous speech produces finals without waiting for recognition feedback.
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};

use super::provider::{
    DualPassConfig, STTProvider, STTProviderType, SttPerformance, TranscriptResult,
};
use super::segmenter::{AudioSegment, Segmenter, SAMPLE_RATE};
use crate::audio::AudioChunk;

// Bound memory and delay if the selected model cannot keep up. Skipped audio
// is counted and displayed, so overload never silently masquerades as realtime.
const MAX_PENDING_SAMPLES: usize = SAMPLE_RATE * 12;

// Two audio sources share model weights, with separate decoder states/languages.
// Serialize access to the shared GPU backend; don't duplicate a 1.6 GB model.
struct SharedWhisper {
    context: whisper_rs::WhisperContext,
    inference: Mutex<()>,
}
static WHISPER_MODELS: OnceLock<Mutex<HashMap<String, Weak<SharedWhisper>>>> = OnceLock::new();

fn shared_context(
    model: &std::path::Path, options: whisper_rs::WhisperContextParameters<'static>,
    backend: &str, device: Option<&str>,
) -> Result<Arc<SharedWhisper>, String> {
    let path = model.canonicalize().map_err(|e| e.to_string())?;
    let metadata = path.metadata().map_err(|e| e.to_string())?;
    let key = format!("{}:{:?}:{}:{backend}:{device:?}", path.display(), metadata.modified().ok(), metadata.len());
    let mut pool = WHISPER_MODELS.get_or_init(|| Mutex::new(HashMap::new())).lock().map_err(|e| e.to_string())?;
    pool.retain(|_, weak| weak.strong_count() > 0);
    if let Some(context) = pool.get(&key).and_then(Weak::upgrade) { return Ok(context); }
    let context = whisper_rs::WhisperContext::new_with_params(path.to_str().ok_or("Invalid model path")?, options)
        .map_err(|e| e.to_string())?;
    let shared = Arc::new(SharedWhisper { context, inference: Mutex::new(()) });
    pool.insert(key, Arc::downgrade(&shared));
    Ok(shared)
}

#[cfg(any(feature = "gpu-vulkan", test))]
struct GpuChoice {
    index: i32,
    name: String,
    discrete: bool,
}

#[cfg(any(feature = "gpu-vulkan", test))]
fn prefer_discrete(mut devices: Vec<GpuChoice>) -> Option<GpuChoice> {
    if devices.is_empty() {
        return None;
    }
    let index = devices
        .iter()
        .position(|device| device.discrete)
        .unwrap_or(0);
    Some(devices.remove(index))
}

/// Used by the desktop and the benchmark so both select the same real GPU.
pub fn context_parameters() -> (
    whisper_rs::WhisperContextParameters<'static>,
    &'static str,
    Option<String>,
) {
    let mut options = whisper_rs::WhisperContextParameters::default();
    options.use_gpu(false);
    #[cfg(feature = "gpu-vulkan")]
    if std::env::var("KAKEHASHI_WHISPER_CPU").as_deref() != Ok("1") {
        use whisper_rs::whisper_rs_sys as sys;
        let mut devices = Vec::new();
        // Registry order is driver dependent: this laptop lists Intel before
        // NVIDIA. Device type, not index 0 or the name, identifies a discrete GPU.
        unsafe {
            sys::ggml_backend_load_all();
            for index in 0..sys::ggml_backend_dev_count() {
                let device = sys::ggml_backend_dev_get(index);
                let kind = sys::ggml_backend_dev_type(device);
                if kind == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU
                    || kind == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU
                {
                    devices.push(GpuChoice {
                        index: devices.len() as i32,
                        name: std::ffi::CStr::from_ptr(sys::ggml_backend_dev_description(device))
                            .to_string_lossy()
                            .into_owned(),
                        discrete: kind == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU,
                    });
                }
            }
        }
        if let Some(device) = prefer_discrete(devices) {
            options
                .use_gpu(true)
                .gpu_device(device.index)
                .flash_attn(true);
            return (options, "Vulkan", Some(device.name));
        }
    }
    (options, "CPU", None)
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

unsafe extern "C" fn inference_cancelled(data: *mut std::ffi::c_void) -> bool {
    unsafe { &*(data as *const AtomicBool) }.load(Ordering::Acquire)
}

fn clean_whisper_text(text: &str) -> String {
    let mut result = String::new();
    let (mut bracket, mut paren) = (0u32, 0u32);
    for ch in text.trim().chars() {
        match ch {
            '[' => bracket += 1,
            ']' if bracket > 0 => bracket -= 1,
            '(' => paren += 1,
            ')' if paren > 0 => paren -= 1,
            _ if bracket > 0 || paren > 0 => (),
            _ => result.push(ch),
        }
    }
    let cleaned = result.trim();
    let lower = cleaned.to_lowercase();
    if cleaned.len() < 2 || lower.contains("blank_audio") || lower.contains("blank audio") {
        String::new()
    } else {
        cleaned.to_string()
    }
}

struct Job {
    audio: AudioSegment,
    id: String,
    queued: Instant,
}

#[derive(Default)]
struct WorkState {
    jobs: VecDeque<Job>,
    pending_samples: usize,
    closing: bool,
    active_end_ms: Option<u64>,
    performance: SttPerformance,
}

impl WorkState {
    fn push(&mut self, job: Job) {
        self.pending_samples += job.audio.samples.len();
        self.jobs.push_back(job);
        while self.pending_samples > MAX_PENDING_SAMPLES && self.jobs.len() > 1 {
            let old = self.jobs.pop_front().unwrap();
            self.pending_samples -= old.audio.samples.len();
            self.performance.dropped_audio_ms += old.audio.duration_ms();
            log::warn!(
                "Whisper overloaded: skipped {} ms of queued audio",
                old.audio.duration_ms()
            );
        }
        self.performance.queued_audio_ms = self.pending_samples as u64 / 16;
    }

    fn pop(&mut self) -> Option<Job> {
        let job = self.jobs.pop_front()?;
        self.pending_samples -= job.audio.samples.len();
        self.performance.queued_audio_ms = self.pending_samples as u64 / 16;
        self.active_end_ms = Some(job.audio.end_ms());
        Some(job)
    }
}

type WorkQueue = Arc<(Mutex<WorkState>, Condvar)>;

pub struct WhisperCppSTT {
    model_path: PathBuf,
    language: String,
    config: Arc<RwLock<DualPassConfig>>,
    segmenter: Segmenter,
    counter: u64,
    streaming: bool,
    work: WorkQueue,
    stop: Arc<AtomicBool>,
    origin_ms: Arc<AtomicU64>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WhisperCppSTT {
    pub fn new(model_path: PathBuf, config: Arc<RwLock<DualPassConfig>>) -> Self {
        Self {
            model_path,
            config,
            language: "en".into(),
            segmenter: Segmenter::default(),
            counter: 0,
            streaming: false,
            work: Arc::new((Mutex::new(WorkState::default()), Condvar::new())),
            stop: Arc::new(AtomicBool::new(false)),
            origin_ms: Arc::new(AtomicU64::new(0)),
            thread: None,
        }
    }

    fn enqueue(&mut self, audio: AudioSegment) {
        self.counter += 1;
        self.work.0.lock().unwrap().push(Job {
            audio,
            id: format!("line_{}", self.counter),
            queued: Instant::now(),
        });
        self.work.1.notify_one();
    }
}

impl Drop for WhisperCppSTT {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.work.1.notify_all();
    }
}

#[async_trait]
impl STTProvider for WhisperCppSTT {
    fn provider_name(&self) -> &str {
        "Whisper.cpp (Local)"
    }
    fn provider_type(&self) -> STTProviderType {
        STTProviderType::WhisperCpp
    }
    fn set_time_origin(&mut self, epoch_ms: u64) {
        self.origin_ms.store(epoch_ms, Ordering::Release);
    }

    fn performance(&self) -> Option<SttPerformance> {
        let state = self.work.0.lock().ok()?;
        let mut metrics = state.performance.clone();
        if let Some(end) = state.active_end_ms {
            metrics.lag_ms = epoch_ms().saturating_sub(end);
        }
        Some(metrics)
    }

    async fn start_stream(
        &mut self,
        result_tx: mpsc::Sender<TranscriptResult>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.streaming {
            return Err("Stream already active".into());
        }
        if !self.model_path.is_file() {
            return Err(format!("Whisper model not found: {}", self.model_path.display()).into());
        }
        self.segmenter = Segmenter::default();
        self.counter = 0;
        self.work = Arc::new((Mutex::new(WorkState::default()), Condvar::new()));
        // Never reset a cancellation flag that a detached old worker still owns.
        self.stop = Arc::new(AtomicBool::new(false));
        self.origin_ms = Arc::new(AtomicU64::new(0));
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
        let model = self.model_path.clone();
        let language = self.language.clone();
        let work = self.work.clone();
        let stop = self.stop.clone();
        let origin_ms = self.origin_ms.clone();
        self.thread = Some(std::thread::spawn(move || {
            use whisper_rs::{FullParams, SamplingStrategy};
            let (options, backend, device) = context_parameters();
            {
                let mut queue = work.0.lock().unwrap();
                queue.performance.backend = backend.into();
                queue.performance.device = device.clone();
            }
            let ctx = match shared_context(&model, options, backend, device.as_deref()) {
                Ok(ctx) => ctx,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Cannot load Whisper: {e}")));
                    return;
                }
            };
            let mut state = match ctx.context.create_state() {
                Ok(state) => state,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("Cannot create Whisper state: {e}")));
                    return;
                }
            };
            if backend == "Vulkan" {
                // Vulkan compiles its pipelines lazily. Warm both encoder and
                // decoder before capture starts, so a first-run shader compile
                // cannot consume the live queue or discard the opening speech.
                log::info!("Preparing Whisper GPU pipelines before capture");
                let mut warmup = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                warmup.set_language(Some(&language));
                warmup.set_n_threads(4);
                warmup.set_no_context(true);
                warmup.set_max_tokens(8);
                warmup.set_print_special(false);
                warmup.set_print_progress(false);
                warmup.set_print_realtime(false);
                warmup.set_print_timestamps(false);
                unsafe {
                    warmup.set_abort_callback(Some(inference_cancelled));
                    warmup
                        .set_abort_callback_user_data(Arc::as_ptr(&stop) as *mut std::ffi::c_void);
                }
                let _inference = ctx.inference.lock().unwrap();
                if let Err(error) = state.full(warmup, &vec![0.0; SAMPLE_RATE]) {
                    let _ = ready_tx.send(Err(format!("Cannot prepare Whisper GPU: {error}")));
                    return;
                }
            }
            origin_ms.store(epoch_ms(), Ordering::Release);
            if ready_tx.send(Ok(())).is_err() {
                return;
            }
            log::info!(
                "Whisper ready: {}, bounded single-pass, 4 host threads",
                work.0.lock().unwrap().performance.backend
            );
            loop {
                let job = {
                    let mut queue = work.0.lock().unwrap();
                    while queue.jobs.is_empty() && !queue.closing && !stop.load(Ordering::Acquire) {
                        queue = work.1.wait(queue).unwrap();
                    }
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    match queue.pop() {
                        Some(job) => job,
                        None => break,
                    }
                };
                let queue_ms = job.queued.elapsed().as_millis() as u64;
                let mut input: Vec<f32> = job
                    .audio
                    .samples
                    .iter()
                    .map(|s| *s as f32 / 32768.0)
                    .collect();
                input.resize(input.len().max(SAMPLE_RATE), 0.0);
                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                params.set_language(Some(&language));
                params.set_n_threads(4);
                params.set_no_context(true);
                params.set_suppress_blank(true);
                params.set_suppress_nst(true);
                params.set_no_speech_thold(0.6);
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);
                // The Arc outlives the synchronous full() call. No per-inference
                // closure allocation or callback pointer escapes this worker.
                unsafe {
                    params.set_abort_callback(Some(inference_cancelled));
                    params
                        .set_abort_callback_user_data(Arc::as_ptr(&stop) as *mut std::ffi::c_void);
                }
                let started = Instant::now();
                let outcome = {
                    let _inference = ctx.inference.lock().unwrap();
                    state.full(params, &input)
                };
                let inference_ms = started.elapsed().as_millis() as u64;
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let mut text = String::new();
                let error = match outcome {
                    Ok(()) => {
                        for i in 0..state.full_n_segments() {
                            if let Some(segment) = state.get_segment(i) {
                                if let Ok(value) = segment.to_str() {
                                    text.push_str(value);
                                }
                            }
                        }
                        None
                    }
                    Err(e) => Some(format!("Whisper inference failed: {e}")),
                };
                {
                    let mut queue = work.0.lock().unwrap();
                    let metrics = &mut queue.performance;
                    metrics.audio_ms = job.audio.duration_ms();
                    metrics.inference_ms = inference_ms;
                    metrics.queue_ms = queue_ms;
                    metrics.lag_ms = epoch_ms().saturating_sub(job.audio.end_ms());
                    metrics.processed_audio_ms += job.audio.duration_ms();
                    metrics.total_inference_ms += inference_ms;
                    metrics.error = error.clone();
                    queue.active_end_ms = None;
                }
                if let Some(error) = error {
                    log::error!("{error}");
                    continue;
                }
                let text = clean_whisper_text(&text);
                if !text.is_empty() {
                    let mut result = TranscriptResult {
                        text,
                        is_final: true,
                        confidence: 0.90,
                        timestamp_ms: job
                            .audio
                            .start_ms
                            .saturating_sub(origin_ms.load(Ordering::Acquire)),
                        duration_ms: Some(job.audio.duration_ms()),
                        speaker: None,
                        language: Some(language.clone()),
                        segment_id: Some(job.id),
                        speaker_tag: None,
                    };
                    // A stalled consumer must not prevent cancellation.
                    loop {
                        match result_tx.try_send(result) {
                            Ok(()) => break,
                            Err(mpsc::error::TrySendError::Full(value))
                                if !stop.load(Ordering::Acquire) =>
                            {
                                result = value;
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            Err(_) => return,
                        }
                    }
                }
            }
        }));
        match ready_rx.await {
            Ok(Ok(())) => {
                self.streaming = true;
                Ok(())
            }
            result => {
                self.stop.store(true, Ordering::Release);
                self.work.1.notify_all();
                if let Some(thread) = self.thread.take() {
                    let _ = thread.join();
                }
                Err(match result {
                    Ok(Err(e)) => e,
                    _ => "Whisper worker exited during startup".into(),
                }
                .into())
            }
        }
    }

    async fn feed_audio(
        &mut self,
        chunk: AudioChunk,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.streaming || self.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if self.thread.as_ref().is_some_and(|t| t.is_finished()) {
            return Err("Whisper worker stopped".into());
        }
        let config = self
            .config
            .read()
            .map_err(|_| "Whisper config lock poisoned")?
            .clone();
        let start_ms = chunk
            .timestamp_ms
            .saturating_sub(chunk.pcm_data.len() as u64 / 16);
        for audio in self.segmenter.push(&chunk.pcm_data, start_ms, &config) {
            self.enqueue(audio);
        }
        Ok(())
    }

    async fn stop_stream(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.streaming {
            return Ok(());
        }
        if let Some(audio) = self.segmenter.flush() {
            self.enqueue(audio);
        }
        self.work.0.lock().unwrap().closing = true;
        self.work.1.notify_all();
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.thread.as_ref().is_some_and(|t| !t.is_finished()) && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.stop.store(true, Ordering::Release);
        self.work.1.notify_all();
        if let Some(thread) = self.thread.take() {
            if thread.is_finished() {
                let _ = thread.join();
            } else {
                log::warn!("Whisper stopped with unfinished audio after the 3s drain limit");
            }
        }
        self.streaming = false;
        Ok(())
    }

    async fn test_connection(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.model_path.is_file())
    }
    fn set_language(&mut self, language: &str) {
        self.language = language.split('-').next().unwrap_or(language).into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_the_discrete_gpu_even_when_intel_is_registered_first() {
        let selected = prefer_discrete(vec![
            GpuChoice {
                index: 0,
                name: "Intel UHD".into(),
                discrete: false,
            },
            GpuChoice {
                index: 1,
                name: "NVIDIA RTX 4050".into(),
                discrete: true,
            },
        ])
        .unwrap();
        assert_eq!(selected.index, 1);
        assert_eq!(selected.name, "NVIDIA RTX 4050");
        assert!(prefer_discrete(Vec::new()).is_none());
    }
    #[test]
    fn overload_is_bounded_counted_and_jobs_remain_chronological() {
        let mut queue = WorkState::default();
        for i in 0..8 {
            queue.push(Job {
                audio: AudioSegment {
                    samples: vec![1000; 48_000],
                    start_ms: i * 3000,
                },
                id: i.to_string(),
                queued: Instant::now(),
            });
        }
        assert_eq!(queue.jobs.len(), 4);
        assert_eq!(queue.performance.dropped_audio_ms, 12_000);
        assert_eq!(queue.performance.queued_audio_ms, 12_000);
        let ids: Vec<String> = std::iter::from_fn(|| queue.pop())
            .map(|job| job.id)
            .collect();
        assert_eq!(ids, ["4", "5", "6", "7"]);
        assert_eq!(queue.performance.queued_audio_ms, 0);
    }

    #[test]
    fn meaningful_greetings_are_not_deleted_as_silence_hallucinations() {
        assert_eq!(clean_whisper_text(" [BLANK_AUDIO] "), "");
        assert_eq!(clean_whisper_text("Thank you."), "Thank you.");
        assert_eq!(clean_whisper_text("こんにちは。"), "こんにちは。");
    }
}
