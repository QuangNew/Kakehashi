// Sub-PRD 4: STT Provider trait and types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::audio::AudioChunk;

/// Result emitted by an STT provider for each recognition event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResult {
    pub text: String,
    pub is_final: bool,
    pub confidence: f32,
    pub timestamp_ms: u64,
    pub speaker: Option<String>,
    pub language: Option<String>,
    /// Optional segment ID — when set, the IPC layer uses this directly
    /// instead of generating one from a counter. Used by dual-pass whisper.cpp.
    #[serde(skip)]
    pub segment_id: Option<String>,
    /// How long the recognized speech lasts (ms from `timestamp_ms`), when
    /// the provider reports it (Deepgram). Used to measure pauses from the
    /// end of a final rather than its start.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Diarization tag of the dominant speaker ("S1", "S2"...), when the
    /// provider separates speakers. `speaker` stays the party label.
    #[serde(default)]
    pub speaker_tag: Option<String>,
}

/// The in-person speaker id ("speaker_0") of a diarization tag ("S1").
pub fn diarized_speaker_id(tag: &str) -> Option<String> {
    let n: u32 = tag.strip_prefix('S')?.parse().ok()?;
    Some(format!("speaker_{}", n.checked_sub(1)?))
}

/// Runtime segmentation preferences. Field names stay compatible with saved settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualPassConfig {
    /// Minimum utterance length, including trailing silence (default 1.0).
    pub short_chunk_secs: f32,
    /// Hard cap for continuous speech, so translation never waits for a long line.
    pub long_chunk_secs: f32,
    /// Actual audio silence before finalizing a segment (default 0.45).
    pub pause_secs: f32,
}

impl Default for DualPassConfig {
    fn default() -> Self {
        Self {
            short_chunk_secs: 1.0,
            long_chunk_secs: 3.0,
            pause_secs: 0.45,
        }
    }
}

impl DualPassConfig {
    pub fn normalized(&self) -> Self {
        fn finite(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                fallback
            }
        }
        let max = finite(self.long_chunk_secs, 3.0, 1.0, 10.0);
        Self {
            short_chunk_secs: finite(self.short_chunk_secs, 1.0, 0.3, max),
            long_chunk_secs: max,
            pause_secs: finite(self.pause_secs, 0.45, 0.2, 2.0),
        }
    }
}

/// Capture-to-text timings; audio discarded under overload is always counted.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SttPerformance {
    pub backend: String,
    pub device: Option<String>,
    pub audio_ms: u64,
    pub inference_ms: u64,
    pub queue_ms: u64,
    pub lag_ms: u64,
    pub queued_audio_ms: u64,
    pub dropped_audio_ms: u64,
    pub processed_audio_ms: u64,
    pub total_inference_ms: u64,
    pub error: Option<String>,
}

/// Configuration common to all STT providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTProviderConfig {
    pub language: String,
    pub api_key: Option<String>,
}

impl Default for STTProviderConfig {
    fn default() -> Self {
        Self {
            language: "en-US".to_string(),
            api_key: None,
        }
    }
}

/// Supported STT provider types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum STTProviderType {
    WindowsNative,
    Deepgram,
    /// Sub-PRD 9: OpenAI Whisper REST API
    WhisperApi,
    /// Sub-PRD 9: Azure Cognitive Services
    AzureSpeech,
    /// Sub-PRD 9: Groq Whisper
    GroqWhisper,
    /// Frontend-only Web Speech API — never instantiated in Rust.
    /// Exists so per-party config can reference it as a valid STT choice.
    WebSpeech,
    /// Local Whisper.cpp engine — runs inference on a dedicated thread.
    WhisperCpp,
    /// Sherpa-ONNX sidecar process — true streaming transducer, fully offline.
    SherpaOnnx,
    /// ONNX Runtime in-process — streaming transducer loaded via ort crate.
    OrtStreaming,
    /// NVIDIA Parakeet TDT — CTC/TDT architecture via ONNX Runtime.
    /// #1 on Open ASR Leaderboard (~6% WER), native streaming.
    ParakeetTdt,
}

impl STTProviderType {
    pub fn as_str(&self) -> &str {
        match self {
            STTProviderType::WindowsNative => "windows_native",
            STTProviderType::Deepgram => "deepgram",
            STTProviderType::WhisperApi => "whisper_api",
            STTProviderType::AzureSpeech => "azure_speech",
            STTProviderType::GroqWhisper => "groq_whisper",
            STTProviderType::WebSpeech => "web_speech",
            STTProviderType::WhisperCpp => "whisper_cpp",
            STTProviderType::SherpaOnnx => "sherpa_onnx",
            STTProviderType::OrtStreaming => "ort_streaming",
            STTProviderType::ParakeetTdt => "parakeet_tdt",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "windows_native" => Some(STTProviderType::WindowsNative),
            "whisper_cpp" => Some(STTProviderType::WhisperCpp),
            "deepgram" => Some(STTProviderType::Deepgram),
            "whisper_api" => Some(STTProviderType::WhisperApi),
            "azure_speech" => Some(STTProviderType::AzureSpeech),
            "groq_whisper" => Some(STTProviderType::GroqWhisper),
            "web_speech" => Some(STTProviderType::WebSpeech),
            "sherpa_onnx" => Some(STTProviderType::SherpaOnnx),
            "ort_streaming" => Some(STTProviderType::OrtStreaming),
            "parakeet_tdt" => Some(STTProviderType::ParakeetTdt),
            _ => None,
        }
    }
}

/// Trait that all STT providers implement.
/// Each provider receives audio chunks and emits transcript results.
#[async_trait]
pub trait STTProvider: Send + Sync {
    fn performance(&self) -> Option<SttPerformance> {
        None
    }
    /// Both audio sources share the same capture clock, including mute gaps.
    fn set_time_origin(&mut self, _epoch_ms: u64) {}

    /// Human-readable name for this provider.
    fn provider_name(&self) -> &str;

    /// The provider type identifier.
    fn provider_type(&self) -> STTProviderType;

    /// Start the recognition stream.
    /// `result_tx` is the channel where TranscriptResults will be sent.
    async fn start_stream(
        &mut self,
        result_tx: mpsc::Sender<TranscriptResult>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Feed an audio chunk to the provider for recognition.
    async fn feed_audio(
        &mut self,
        chunk: AudioChunk,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Stop the recognition stream and clean up resources.
    async fn stop_stream(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Test whether the provider is available/configured correctly.
    async fn test_connection(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Set the recognition language (e.g., "en-US", "es-ES").
    fn set_language(&mut self, language: &str);
}

/// Provider info for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTProviderInfo {
    pub provider_type: String,
    pub name: String,
    pub requires_api_key: bool,
    pub is_local: bool,
    pub supported_languages: Vec<String>,
}

/// List of available providers with metadata.
pub fn list_available_providers() -> Vec<STTProviderInfo> {
    vec![
        STTProviderInfo {
            provider_type: "windows_native".to_string(),
            name: "Windows Speech".to_string(),
            requires_api_key: false,
            is_local: true,
            // Availability depends on the speech languages installed in Windows.
            supported_languages: vec![],
        },
        STTProviderInfo {
            provider_type: "web_speech".to_string(),
            name: "Web Speech API".to_string(),
            requires_api_key: false,
            is_local: true,
            supported_languages: vec![
                "en-US".to_string(),
                "en-GB".to_string(),
                "es-ES".to_string(),
                "fr-FR".to_string(),
                "de-DE".to_string(),
                "it-IT".to_string(),
                "pt-BR".to_string(),
                "ja-JP".to_string(),
                "zh-CN".to_string(),
                "ko-KR".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "whisper_cpp".to_string(),
            name: "Whisper.cpp (Local)".to_string(),
            requires_api_key: false,
            is_local: true,
            supported_languages: vec![
                "en-US".to_string(),
                "en-GB".to_string(),
                "es-ES".to_string(),
                "fr-FR".to_string(),
                "de-DE".to_string(),
                "it-IT".to_string(),
                "pt-BR".to_string(),
                "ja-JP".to_string(),
                "zh-CN".to_string(),
                "ko-KR".to_string(),
                "nl-NL".to_string(),
                "hi-IN".to_string(),
                "ru-RU".to_string(),
                "ar-SA".to_string(),
                "tr-TR".to_string(),
                "pl-PL".to_string(),
                "sv-SE".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "deepgram".to_string(),
            name: "Deepgram".to_string(),
            requires_api_key: true,
            is_local: false,
            supported_languages: vec![
                "en".to_string(),
                "en-US".to_string(),
                "en-GB".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "it".to_string(),
                "pt".to_string(),
                "ja".to_string(),
                "zh".to_string(),
                "ko".to_string(),
                "nl".to_string(),
                "hi".to_string(),
                "ru".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "whisper_api".to_string(),
            name: "Whisper API (OpenAI)".to_string(),
            requires_api_key: true,
            is_local: false,
            supported_languages: vec![
                "en".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "it".to_string(),
                "pt".to_string(),
                "ja".to_string(),
                "zh".to_string(),
                "ko".to_string(),
                "nl".to_string(),
                "hi".to_string(),
                "ru".to_string(),
                "ar".to_string(),
                "tr".to_string(),
                "pl".to_string(),
                "sv".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "azure_speech".to_string(),
            name: "Azure Speech".to_string(),
            requires_api_key: true,
            is_local: false,
            supported_languages: vec![
                "en-US".to_string(),
                "en-GB".to_string(),
                "es-ES".to_string(),
                "fr-FR".to_string(),
                "de-DE".to_string(),
                "it-IT".to_string(),
                "pt-BR".to_string(),
                "ja-JP".to_string(),
                "zh-CN".to_string(),
                "ko-KR".to_string(),
                "nl-NL".to_string(),
                "hi-IN".to_string(),
                "ru-RU".to_string(),
                "ar-SA".to_string(),
                "tr-TR".to_string(),
                "pl-PL".to_string(),
                "sv-SE".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "groq_whisper".to_string(),
            name: "Groq Whisper".to_string(),
            requires_api_key: true,
            is_local: false,
            supported_languages: vec![
                "en".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "it".to_string(),
                "pt".to_string(),
                "ja".to_string(),
                "zh".to_string(),
                "ko".to_string(),
                "nl".to_string(),
                "hi".to_string(),
                "ru".to_string(),
                "ar".to_string(),
                "tr".to_string(),
                "pl".to_string(),
                "sv".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "sherpa_onnx".to_string(),
            name: "Sherpa-ONNX (Local, Streaming)".to_string(),
            requires_api_key: false,
            is_local: true,
            supported_languages: vec![
                "en".to_string(),
                "zh".to_string(),
                "de".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "ja".to_string(),
                "ko".to_string(),
                "ru".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "ort_streaming".to_string(),
            name: "ORT Streaming (Local, GPU)".to_string(),
            requires_api_key: false,
            is_local: true,
            supported_languages: vec![
                "en".to_string(),
                "zh".to_string(),
                "de".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "ja".to_string(),
                "ko".to_string(),
                "ru".to_string(),
            ],
        },
        STTProviderInfo {
            provider_type: "parakeet_tdt".to_string(),
            name: "Parakeet TDT (Best Local)".to_string(),
            requires_api_key: false,
            is_local: true,
            supported_languages: vec![
                "bg".to_string(),
                "hr".to_string(),
                "cs".to_string(),
                "da".to_string(),
                "nl".to_string(),
                "en".to_string(),
                "et".to_string(),
                "fi".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "el".to_string(),
                "hu".to_string(),
                "it".to_string(),
                "lv".to_string(),
                "lt".to_string(),
                "mt".to_string(),
                "pl".to_string(),
                "pt".to_string(),
                "ro".to_string(),
                "sk".to_string(),
                "sl".to_string(),
                "es".to_string(),
                "sv".to_string(),
                "ru".to_string(),
                "uk".to_string(),
            ],
        },
    ]
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn every_selectable_stt_provider_is_advertised_and_can_be_resolved() {
        let catalog = list_available_providers();
        for provider in [STTProviderType::WindowsNative, STTProviderType::WebSpeech,
            STTProviderType::WhisperCpp, STTProviderType::Deepgram, STTProviderType::WhisperApi,
            STTProviderType::AzureSpeech, STTProviderType::GroqWhisper, STTProviderType::SherpaOnnx,
            STTProviderType::OrtStreaming, STTProviderType::ParakeetTdt] {
            assert_eq!(STTProviderType::from_str(provider.as_str()), Some(provider.clone()));
            assert!(catalog.iter().any(|entry| entry.provider_type == provider.as_str()), "Missing {}", provider.as_str());
        }
    }
}
