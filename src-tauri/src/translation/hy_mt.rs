use super::*;
use crate::local_runtime::{LocalRuntime, ModelRole};
use std::time::Duration;

/// Compile local GPU kernels before capturing live speech. Cloud providers are
/// never called for warmup. Discard these neutral examples instead of saving them.
pub async fn prepare_local_translation(
    provider: &dyn TranslationProvider,
) -> Result<(), TranslationError> {
    if provider.provider_type() != TranslationProviderType::HyMt {
        return Ok(());
    }
    provider.prepare().await
}

pub struct HyMtTranslator {
    runtime: Arc<LocalRuntime>,
    client: reqwest::Client,
    prepared_pid: tokio::sync::Mutex<Option<u32>>,
}

impl HyMtTranslator {
    pub fn new(runtime: Arc<LocalRuntime>) -> Self {
        Self {
            prepared_pid: tokio::sync::Mutex::new(None),
            runtime,
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(90))
                .build()
                .expect("local HTTP client"),
        }
    }
}

#[async_trait]
impl TranslationProvider for HyMtTranslator {
    async fn prepare(&self) -> Result<(),TranslationError> {
        let mut prepared=self.prepared_pid.lock().await;
        let endpoint=self.runtime.ensure(ModelRole::Translation).await.map_err(TranslationError::NotConfigured)?;
        let pid=self.runtime.status().iter().find(|s|s.role==ModelRole::Translation).and_then(|s|s.pid);
        if pid.is_some() && *prepared==pid { return Ok(()); }
        // Neutral, capped warmup exercises encoder/decoder shapes, without
        // generating two long paragraphs before every meeting.
        for (text,target) in [("こんにちは。","Vietnamese"),("Xin chào.","Japanese")] {
            let response=self.client.post(format!("{endpoint}/chat/completions")).json(&serde_json::json!({
                "model":"translation","stream":false,"messages":[{"role":"user","content":format!("Translate into {target}. Output only the translation.\n{text}")}],
                "temperature":0.0,"max_tokens":8,
            })).send().await.map_err(|e|TranslationError::Http(e.to_string()))?;
            if !response.status().is_success(){return Err(TranslationError::Failed("Local translation warmup failed".into()));}
            let _=response.bytes().await.map_err(|e|TranslationError::Http(e.to_string()))?;
        }
        *prepared=pid;Ok(())
    }
    fn provider_name(&self) -> &str {
        "Hy-MT Local"
    }
    fn provider_type(&self) -> TranslationProviderType {
        TranslationProviderType::HyMt
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn translate(
        &self,
        text: &str,
        source: Option<&str>,
        target: &str,
    ) -> Result<String, TranslationError> {
        self.translate_with_context(text, source, target, &[]).await
    }
    async fn translate_with_context(
        &self, text: &str, source: Option<&str>, target: &str,
        context: &[TranslationContextTurn],
    ) -> Result<String, TranslationError> {
        let prompt = super::prompt::local_prompt(text, source, target, context)?;
        let runtime = self.runtime.clone();
        let endpoint = tokio::spawn(async move { runtime.ensure(ModelRole::Translation).await })
            .await
            .map_err(|e| TranslationError::Failed(e.to_string()))?
            .map_err(TranslationError::NotConfigured)?;
        let response = self.client.post(format!("{}/chat/completions", endpoint)).json(&serde_json::json!({
            "model": "translation", "stream": false,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.7, "top_p": 0.6, "top_k": 20, "repeat_penalty": 1.05, "max_tokens": 512
        })).send().await.map_err(|e| TranslationError::Http(e.to_string()))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| TranslationError::Failed(e.to_string()))?;
        if !status.is_success() {
            return Err(TranslationError::Failed(format!(
                "Local translation server ({}): {}",
                status, body
            )));
        }
        let choice = body["choices"]
            .get(0)
            .ok_or_else(|| TranslationError::Failed("No translation returned".into()))?;
        if choice["finish_reason"] == "length" {
            return Err(TranslationError::Failed(
                "Translation exceeded its output limit; use a shorter transcript segment".into(),
            ));
        }
        let translated = choice["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .trim();
        if translated.is_empty() {
            return Err(TranslationError::Failed(
                "Local model returned an empty translation".into(),
            ));
        }
        Ok(translated.to_owned())
    }
    async fn detect_language(&self, _text: &str) -> Result<DetectedLanguage, TranslationError> {
        Err(TranslationError::NotConfigured(
            "Choose the source language for each audio source".into(),
        ))
    }
    async fn supported_languages(&self) -> Result<Vec<Language>, TranslationError> {
        Ok(vec![
            Language {
                code: "vi".into(),
                name: "Vietnamese".into(),
                native_name: Some("Tiếng Việt".into()),
            },
            Language {
                code: "ja".into(),
                name: "Japanese".into(),
                native_name: Some("日本語".into()),
            },
            Language {
                code: "en".into(),
                name: "English".into(),
                native_name: Some("English".into()),
            },
        ])
    }
    async fn test_connection(&self) -> Result<ConnectionStatus, TranslationError> {
        let start = std::time::Instant::now();
        self.runtime
            .ensure(ModelRole::Translation)
            .await
            .map_err(TranslationError::NotConfigured)?;
        Ok(ConnectionStatus {
            connected: true,
            language_count: 3,
            response_ms: start.elapsed().as_millis() as u64,
            error: None,
        })
    }
}
