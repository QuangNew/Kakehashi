use super::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// A dedicated translation client. It never reads or changes the copilot router.
pub struct ApiTranslator {
    kind: TranslationProviderType,
    model: String,
    key: String,
    base: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
pub struct ApiModel {
    pub id: String,
    pub name: String,
}

impl ApiTranslator {
    pub fn new(
        kind: TranslationProviderType,
        model: String,
        key: String,
    ) -> Result<Self, TranslationError> {
        let base = match kind {
            TranslationProviderType::Groq => "https://api.groq.com/openai/v1",
            TranslationProviderType::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            _ => {
                return Err(TranslationError::NotConfigured(
                    "Choose Groq or Google AI Studio".into(),
                ))
            }
        };
        let model = model
            .trim()
            .strip_prefix("models/")
            .unwrap_or(model.trim())
            .to_owned();
        if model.is_empty()
            || model.len() > 200
            || !model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-._/".contains(c))
            || (kind == TranslationProviderType::Gemini && model.contains('/'))
        {
            return Err(TranslationError::NotConfigured(
                "Choose a valid translation model".into(),
            ));
        }
        if key.trim().is_empty() {
            return Err(TranslationError::NoApiKey(kind.to_string()));
        }
        Ok(Self {
            kind,
            model,
            key,
            base: base.into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .map_err(|e| TranslationError::Http(e.to_string()))?,
        })
    }

    fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.kind == TranslationProviderType::Gemini {
            request.header("x-goog-api-key", &self.key)
        } else {
            request.bearer_auth(&self.key)
        }
    }

    async fn json(&self, request: reqwest::RequestBuilder) -> Result<Value, TranslationError> {
        let response = self
            .auth(request)
            .send()
            .await
            .map_err(|e| TranslationError::Http(e.to_string()))?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| TranslationError::Failed(format!("Invalid API response: {e}")))?;
        if !status.is_success() {
            let detail = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("API request failed");
            // Do not expose credentials even if a proxy/provider echoes them in an error.
            let detail = detail
                .replace(&self.key, "[redacted]")
                .chars()
                .take(500)
                .collect::<String>();
            return Err(TranslationError::Failed(format!(
                "{} API ({}): {}",
                self.kind,
                status.as_u16(),
                detail
            )));
        }
        Ok(body)
    }

    pub async fn models(
        kind: TranslationProviderType,
        key: String,
    ) -> Result<Vec<ApiModel>, TranslationError> {
        let instance = Self::new(kind, "catalog".into(), key)?;
        instance.list_models().await
    }

    async fn list_models(&self) -> Result<Vec<ApiModel>, TranslationError> {
        let mut out = Vec::new();
        let mut page = String::new();
        for _ in 0..10 {
            let mut request = self.client.get(format!("{}/models", self.base));
            if self.kind == TranslationProviderType::Gemini {
                request = request.query(&[("pageSize", "1000"), ("pageToken", page.as_str())]);
            }
            let body = self.json(request).await?;
            let values = if self.kind == TranslationProviderType::Gemini {
                body.get("models")
            } else {
                body.get("data")
            };
            if let Some(values) = values.and_then(Value::as_array) {
                for value in values {
                    let id = if self.kind == TranslationProviderType::Gemini {
                        let supported = value
                            .get("supportedGenerationMethods")
                            .and_then(Value::as_array)
                            .map(|methods| {
                                methods
                                    .iter()
                                    .any(|method| method.as_str() == Some("generateContent"))
                            })
                            .unwrap_or(false);
                        if !supported {
                            continue;
                        }
                        value
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim_start_matches("models/")
                    } else {
                        value.get("id").and_then(Value::as_str).unwrap_or("")
                    };
                    let lower = id.to_ascii_lowercase();
                    if id.is_empty()
                        || value.get("active").and_then(Value::as_bool) == Some(false)
                        || ["whisper", "tts", "orpheus", "embed", "guard", "playai"]
                            .iter()
                            .any(|name| lower.contains(name))
                    {
                        continue;
                    }
                    out.push(ApiModel {
                        id: id.into(),
                        name: value
                            .get("displayName")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .into(),
                    });
                }
            }
            page = body
                .get("nextPageToken")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into();
            if self.kind != TranslationProviderType::Gemini || page.is_empty() {
                break;
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out.dedup_by(|a, b| a.id == b.id);
        Ok(out)
    }

    fn output(&self, body: &Value) -> Result<String, TranslationError> {
        let text = if self.kind == TranslationProviderType::Groq {
            if body
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str)
                == Some("length")
            {
                return Err(TranslationError::Failed(
                    "Translation exceeded the model output limit; shorten the segment".into(),
                ));
            }
            body.pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        } else {
            if body
                .pointer("/candidates/0/finishReason")
                .and_then(Value::as_str)
                == Some("MAX_TOKENS")
            {
                return Err(TranslationError::Failed(
                    "Translation exceeded the model output limit; shorten the segment".into(),
                ));
            }
            body.pointer("/candidates/0/content/parts")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default()
        };
        if text.trim().is_empty() {
            return Err(TranslationError::Failed(
                "API returned no translation (check the selected model or blocked response)".into(),
            ));
        }
        Ok(text.trim().to_owned())
    }
}

#[async_trait]
impl TranslationProvider for ApiTranslator {
    fn provider_name(&self) -> &str {
        if self.kind == TranslationProviderType::Groq {
            "Groq Translation"
        } else {
            "Google AI Studio Translation"
        }
    }
    fn provider_type(&self) -> TranslationProviderType {
        self.kind.clone()
    }
    fn is_local(&self) -> bool {
        false
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
        let instruction = super::prompt::instruction(source, target);
        let text = super::prompt::content(text, context);
        let request = if self.kind == TranslationProviderType::Groq {
            self.client.post(format!("{}/chat/completions", self.base)).json(&json!({
                "model": self.model, "messages": [{"role":"system","content":instruction},{"role":"user","content":text}],
                "temperature": 0.1, "max_completion_tokens": 4096, "stream": false
            }))
        } else {
            self.client
                .post(format!(
                    "{}/models/{}:generateContent",
                    self.base, self.model
                ))
                .json(&json!({
                    "systemInstruction":{"parts":[{"text":instruction}]},
                    "contents":[{"role":"user","parts":[{"text":text}]}],
                    "generationConfig":{"temperature":0.1,"maxOutputTokens":4096}
                }))
        };
        self.output(&self.json(request).await?)
    }
    async fn detect_language(&self, _text: &str) -> Result<DetectedLanguage, TranslationError> {
        Err(TranslationError::NotConfigured(
            "Select the source language or use the bilingual source routing".into(),
        ))
    }
    async fn supported_languages(&self) -> Result<Vec<Language>, TranslationError> {
        Ok(
            [("vi", "Vietnamese"), ("ja", "Japanese"), ("en", "English")]
                .into_iter()
                .map(|(code, name)| Language {
                    code: code.into(),
                    name: name.into(),
                    native_name: None,
                })
                .collect(),
        )
    }
    async fn test_connection(&self) -> Result<ConnectionStatus, TranslationError> {
        let started = Instant::now();
        let models = self.list_models().await?;
        if !models.iter().any(|model| model.id == self.model) {
            return Err(TranslationError::NotConfigured(
                "Selected model is not in this API key's text model catalog".into(),
            ));
        }
        Ok(ConnectionStatus {
            connected: true,
            language_count: 3,
            response_ms: started.elapsed().as_millis() as u64,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn server(
        responses: Vec<(u16, Value)>,
    ) -> (String, tokio::task::JoinHandle<Vec<(String, Value)>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                let header_end = loop {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    assert!(bytes.len() < 65_536);
                    if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break index + 4;
                    }
                };
                let header = String::from_utf8_lossy(&bytes[..header_end]).to_string();
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                }
                let request = if length > 0 {
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
                } else {
                    Value::Null
                };
                requests.push((header, request));
                let body = body.to_string();
                socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
            requests
        });
        (base, task)
    }
    fn client(kind: TranslationProviderType, base: String) -> ApiTranslator {
        let mut client = ApiTranslator::new(
            kind,
            "models/translation-model".into(),
            "translation-test-key".into(),
        )
        .unwrap();
        client.base = base;
        client.client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        client
    }

    #[tokio::test]
    async fn groq_posts_translation_only_with_its_selected_model_and_bearer_key() {
        let (base, requests) = server(vec![(
            200,
            json!({"choices":[{"message":{"content":" Chào bạn "},"finish_reason":"stop"}]}),
        )])
        .await;
        let client = client(TranslationProviderType::Groq, base);
        assert_eq!(
            client
                .translate("こんにちは", Some("ja"), "vi")
                .await
                .unwrap(),
            "Chào bạn"
        );
        let requests = requests.await.unwrap();
        let (header, body) = &requests[0];
        assert!(header.starts_with("POST /chat/completions "));
        assert!(header
            .to_ascii_lowercase()
            .contains("authorization: bearer translation-test-key"));
        assert!(!header.lines().next().unwrap().contains("key"));
        assert_eq!(body["model"], "translation-model");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][1]["content"], "CURRENT text:\nこんにちは");
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("Never answer"));
    }

    #[tokio::test]
    async fn context_is_sent_as_reference_while_the_current_text_remains_separate() {
        let (base, requests) = server(vec![(200, json!({"choices":[{"message":{"content":"まだ確約できません。"},"finish_reason":"stop"}]}))]).await;
        let client = client(TranslationProviderType::Groq, base);
        let context = [TranslationContextTurn { source_text: "Về hạn giao thứ sáu.".into(), translated_text: "金曜日の納期についてです。".into() }];
        client.translate_with_context("Bên em chưa chốt được.", Some("vi"), "ja", &context).await.unwrap();
        let requests = requests.await.unwrap();
        let body = &requests[0].1;
        let text = body["messages"][1]["content"].as_str().unwrap();
        assert!(text.contains("Previous speech (reference only)"));
        assert!(text.ends_with("CURRENT text:\nBên em chưa chốt được."));
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("Japanese"));
    }

    #[tokio::test]
    async fn gemini_uses_header_auth_and_hides_thinking_from_the_subtitle() {
        let (base,requests) = server(vec![(200,json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"private reasoning","thought":true},{"text":"こんにちは"}]}}]}))]).await;
        let client = client(TranslationProviderType::Gemini, base);
        assert_eq!(
            client
                .translate("Xin chào", Some("vi"), "ja")
                .await
                .unwrap(),
            "こんにちは"
        );
        let requests = requests.await.unwrap();
        let (header, body) = &requests[0];
        assert!(header.starts_with("POST /models/translation-model:generateContent "));
        assert!(header
            .to_ascii_lowercase()
            .contains("x-goog-api-key: translation-test-key"));
        assert!(!header
            .lines()
            .next()
            .unwrap()
            .contains("translation-test-key"));
        assert_eq!(body["contents"][0]["parts"][0]["text"], "CURRENT text:\nXin chào");
        assert!(body.get("systemInstruction").is_some());
        assert!(body.get("cachedContent").is_none());
    }

    #[tokio::test]
    async fn gemini_catalog_follows_pagination_and_keeps_text_generation_models() {
        let (base,requests) = server(vec![
            (200,json!({"models":[{"name":"models/embedding-only","supportedGenerationMethods":["embedContent"]}],"nextPageToken":"next-page"})),
            (200,json!({"models":[{"name":"models/translation-model","displayName":"Translation","supportedGenerationMethods":["generateContent"]}]})),
        ]).await;
        let models = client(TranslationProviderType::Gemini, base)
            .list_models()
            .await
            .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "translation-model");
        let requests = requests.await.unwrap();
        assert!(requests[1]
            .0
            .lines()
            .next()
            .unwrap()
            .contains("pageToken=next-page"));
    }

    #[tokio::test]
    async fn rate_limit_is_reported_once_and_never_exposes_the_key() {
        let (base, requests) = server(vec![(
            429,
            json!({"error":{"message":"Rate limit for translation-test-key"}}),
        )])
        .await;
        let error = client(TranslationProviderType::Groq, base)
            .translate("a", Some("vi"), "ja")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("429") && error.contains("[redacted]"));
        assert!(!error.contains("translation-test-key"));
        assert_eq!(requests.await.unwrap().len(), 1);
    }

    #[test]
    fn rejects_truncated_empty_or_invalid_model_output() {
        let groq = client(TranslationProviderType::Groq, "http://127.0.0.1".into());
        assert!(groq
            .output(
                &json!({"choices":[{"finish_reason":"length","message":{"content":"partial"}}]})
            )
            .is_err());
        let gemini = client(TranslationProviderType::Gemini, "http://127.0.0.1".into());
        assert!(gemini
            .output(&json!({"candidates":[{"finishReason":"MAX_TOKENS"}]}))
            .is_err());
        assert!(gemini.output(&json!({"candidates":[]})).is_err());
        assert!(ApiTranslator::new(
            TranslationProviderType::Gemini,
            "../bad?key=leak".into(),
            "test".into()
        )
        .is_err());
        assert!(
            ApiTranslator::new(TranslationProviderType::Groq, "model".into(), " ".into()).is_err()
        );
    }
}
