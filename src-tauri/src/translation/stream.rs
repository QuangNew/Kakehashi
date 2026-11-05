use super::TranslationError;
use futures::StreamExt;

#[derive(Default)]
struct TranslationStream {
    pending: Vec<u8>,
    text: String,
    stopped: bool,
}

impl TranslationStream {
    fn feed(&mut self, bytes: &[u8]) -> Result<bool, TranslationError> {
        self.pending.extend_from_slice(bytes);
        let mut changed = false;
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let line: Vec<_> = self.pending.drain(..=end).collect();
            let line = std::str::from_utf8(&line).map_err(|e| TranslationError::Failed(e.to_string()))?;
            let Some(data) = line.trim().strip_prefix("data:").map(str::trim) else { continue; };
            if data == "[DONE]" || data.is_empty() { continue; }
            let data: serde_json::Value = serde_json::from_str(data)
                .map_err(|e| TranslationError::Failed(format!("Invalid translation stream: {e}")))?;
            if data.get("error").is_some() {
                return Err(TranslationError::Failed("Local translation stream failed".into()));
            }
            let Some(choice) = data["choices"].get(0) else { continue; };
            if choice["finish_reason"] == "length" {
                return Err(TranslationError::Failed("Translation exceeded its output limit; use a shorter transcript segment".into()));
            }
            if choice["finish_reason"] == "stop" { self.stopped = true; }
            if let Some(token) = choice["delta"]["content"].as_str() {
                self.text.push_str(token);
                changed |= !token.is_empty();
            }
        }
        if self.pending.len() > 1024 * 1024 || self.text.len() > 64 * 1024 {
            return Err(TranslationError::Failed("Translation stream exceeded its size limit".into()));
        }
        Ok(changed)
    }

    fn finish(self) -> Result<String, TranslationError> {
        if !self.stopped {
            return Err(TranslationError::Failed("Translation stream interrupted before completion. Please retry.".into()));
        }
        let text = self.text.trim();
        if text.is_empty() { return Err(TranslationError::Failed("Local model returned an empty translation".into())); }
        Ok(text.to_owned())
    }
}

pub async fn read(
    response: reqwest::Response,
    progress: &(dyn for<'text> Fn(&'text str) + Send + Sync),
) -> Result<String, TranslationError> {
    let mut bytes = response.bytes_stream();
    let mut parser = TranslationStream::default();
    let mut last_emit = None::<std::time::Instant>;
    while let Some(chunk) = bytes.next().await {
        let chunk = chunk.map_err(|e| TranslationError::Http(e.to_string()))?;
        if parser.feed(&chunk)? && last_emit.is_none_or(|at| at.elapsed().as_millis() >= 50) {
            progress(parser.text.trim());
            last_emit = Some(std::time::Instant::now());
        }
    }
    parser.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_tokens_survive_every_network_byte_boundary() {
        let mut stream = TranslationStream::default();
        let wire = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"こんにちは。\"},\"finish_reason\":null}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Xin chào!\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
        );
        for byte in wire.as_bytes() { stream.feed(&[*byte]).unwrap(); }
        assert_eq!(stream.finish().unwrap(), "こんにちは。Xin chào!");
    }
    #[test]
    fn interrupted_or_length_limited_output_is_never_committed_as_a_complete_translation() {
        let mut stream = TranslationStream::default();
        stream.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n").unwrap();
        assert!(stream.finish().is_err());
        let mut stream = TranslationStream::default();
        assert!(stream.feed(b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n").is_err());
    }
}
