// HTTP download with streaming progress and SHA256 verification.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;

static REVISION: AtomicU64 = AtomicU64::new(1);
pub fn next_revision() -> u64 { REVISION.fetch_add(1, Ordering::Relaxed) }

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub engine: String,
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub status: String, // "downloading" | "verifying" | "complete" | "error" | "cancelled"
    pub revision: u64,
    pub error: Option<String>,
}

/// Download into a resumable partial file. Never promote a truncated body.
/// Progress is supplied by the manager so events and snapshots share one state.
pub async fn download_file(
    url: &str,
    dest: &Path,
    sha256_expected: &str,
    cancel: Arc<AtomicBool>,
    report: impl Fn(u64, u64, &str) + Send + Sync,
) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) { return Err("Download cancelled".into()); }
    let work = async {
        use sha2::{Digest, Sha256};
        use tokio::io::AsyncReadExt;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .build().map_err(|e| e.to_string())?;
        let partial = dest.with_extension("download");
        let mut completed = None;
        let mut last_error = String::new();
        for attempt in 0..5u32 {
            match transfer(&client, url, &partial, sha256_expected, &cancel, &report).await {
                Ok(result) => { completed = Some(result); break; }
                Err(error) => {
                    if cancel.load(Ordering::Acquire) { return Err("Download cancelled".into()); }
                    if !error.retryable { return Err(error.message); }
                    last_error = error.message;
                    if attempt == 4 { break; }
                    let bytes = tokio::fs::metadata(&partial).await.map(|m| m.len()).unwrap_or(0);
                    report(bytes, 0, "retrying");
                    let delay = if cfg!(test) { 5 } else { 500 * (1u64 << attempt) };
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(delay)) => (),
                        _ = cancelled(&cancel) => return Err("Download cancelled".into()),
                    }
                }
            }
        }
        let (downloaded, total) = completed.ok_or(last_error)?;
        if !sha256_expected.is_empty() {
            report(downloaded, total, "verifying");
            let mut file = tokio::fs::File::open(&partial).await.map_err(|e| e.to_string())?;
            let mut hasher = Sha256::new();
            let mut buffer = vec![0; 1024 * 1024];
            loop {
                if cancel.load(Ordering::Acquire) { return Err("Download cancelled".into()); }
                let count = file.read(&mut buffer).await.map_err(|e| e.to_string())?;
                if count == 0 { break; }
                hasher.update(&buffer[..count]);
            }
            if format!("{:x}", hasher.finalize()) != sha256_expected.to_ascii_lowercase() {
                drop(file);
                // Keep the rejected bytes for diagnosis, but do not resume them again.
                let rejected = partial.with_extension(format!("download.invalid-{}", uuid::Uuid::new_v4()));
                tokio::fs::rename(&partial, rejected).await.map_err(|e| e.to_string())?;
                return Err("Model checksum mismatch; the incomplete file was not installed. Retry to download a clean copy.".into());
            }
        }
        if cancel.load(Ordering::Acquire) { return Err("Download cancelled".into()); }
        tokio::fs::rename(&partial, dest).await.map_err(|e| format!("Could not install model: {e}"))?;
        let _ = tokio::fs::remove_file(partial.with_extension("download.json")).await;
        Ok(())
    };
    work.await
}

async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Acquire) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PartialMetadata { url: String, validator: Option<String> }

struct TransferError {
    message: String,
    retryable: bool,
}

impl From<String> for TransferError {
    fn from(message: String) -> Self { Self { message, retryable: true } }
}

impl From<&str> for TransferError {
    fn from(message: &str) -> Self { message.to_owned().into() }
}

async fn transfer(
    client: &reqwest::Client, url: &str, partial: &Path, sha256: &str, cancel: &AtomicBool,
    report: &(impl Fn(u64, u64, &str) + Send + Sync),
) -> Result<(u64, u64), TransferError> {
    use futures::StreamExt;
    use reqwest::header::{ACCEPT_ENCODING, CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE};
    use tokio::io::AsyncWriteExt;
    use std::time::{Duration, Instant};
    let metadata_path = partial.with_extension("download.json");
    let metadata = tokio::fs::read(&metadata_path).await.ok()
        .and_then(|data| serde_json::from_slice::<PartialMetadata>(&data).ok());
    let mut offset = tokio::fs::metadata(partial).await.map(|m| m.len()).unwrap_or(0);
    // Legacy Whisper partials can be resumed safely using the pinned full-file hash.
    if sha256.is_empty() && !metadata.as_ref().is_some_and(|m| m.url == url && m.validator.is_some()) {
        offset = 0;
    }
    let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
        if let Some(validator) = metadata.as_ref().filter(|m| m.url == url).and_then(|m| m.validator.as_ref()) {
            request = request.header(IF_RANGE, validator);
        }
    }
    report(offset, 0, "downloading");
    let response = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(45), request.send()) => result
            .map_err(|_| "Download response timed out".to_string())?
            .map_err(|e| format!("Download request failed: {e}"))?,
        _ = cancelled(cancel) => return Err("Download cancelled".into()),
    };
    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        let total = response.headers().get(CONTENT_RANGE).and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("bytes */")).and_then(|s| s.parse::<u64>().ok());
        if total == Some(offset) && offset > 0 { return Ok((offset, offset)); }
        if total.is_some_and(|n| offset > n) {
            tokio::fs::OpenOptions::new().write(true).open(partial).await
                .map_err(|e| e.to_string())?.set_len(0).await.map_err(|e| e.to_string())?;
        }
        return Err("Server rejected the partial download range; retrying".into());
    }
    if !response.status().is_success() {
        let status = response.status();
        let hint = match status {
            reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE =>
                "The model file is unavailable at this address. Update the app or check the model source.",
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN =>
                "The server denied access to the model file.",
            _ => "The server could not provide the model file.",
        };
        return Err(TransferError {
            message: format!("Model download returned HTTP {status}. {hint}"),
            retryable: status.is_server_error() || matches!(status.as_u16(), 408 | 425 | 429),
        });
    }
    let total;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let range = response.headers().get(CONTENT_RANGE).and_then(|h| h.to_str().ok())
            .and_then(parse_content_range).ok_or("Invalid download Content-Range")?;
        if range.0 != offset || range.1 < range.0 || range.1 >= range.2 {
            return Err("Server returned a mismatched download range".into());
        }
        total = range.2;
    } else {
        // A server may ignore Range/If-Range. Restart instead of appending a full file.
        offset = 0;
        total = response.content_length().unwrap_or(0);
    }
    let validator = response.headers().get(ETAG).or_else(|| response.headers().get(LAST_MODIFIED))
        .and_then(|h| h.to_str().ok()).map(str::to_owned);
    let metadata = PartialMetadata { url: url.into(), validator };
    tokio::fs::write(&metadata_path, serde_json::to_vec(&metadata).map_err(|e| e.to_string())?)
        .await.map_err(|e| e.to_string())?;
    let mut file = tokio::fs::OpenOptions::new().create(true).write(true)
        .truncate(offset == 0).append(offset > 0).open(partial).await.map_err(|e| e.to_string())?;
    let mut downloaded = offset;
    let mut stream = response.bytes_stream();
    let mut last_report = Instant::now();
    report(downloaded, total, "downloading");
    let streamed = async {
      loop {
        let next = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(45), stream.next()) => result
                .map_err(|_| "Download stream stalled; partial data retained".to_string())?,
            _ = cancelled(cancel) => return Err("Download cancelled".to_string()),
        };
        let Some(chunk) = next else { break; };
        let chunk = chunk.map_err(|e| format!("Download stream interrupted: {e}"))?;
        if total > 0 && downloaded + chunk.len() as u64 > total {
            return Err("Server sent more bytes than the declared model size".into());
        }
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if last_report.elapsed() >= Duration::from_millis(250) || downloaded == total {
            report(downloaded, total, "downloading");
            last_report = Instant::now();
        }
      }
      Ok::<_, String>(())
    }.await;
    // Complete pending writes before a retry can open the same partial file.
    file.flush().await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    streamed?;
    if total > 0 && downloaded != total {
        return Err("Download ended early; partial data retained".into());
    }
    report(downloaded, total, "downloading");
    Ok((downloaded, total))
}

fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("kakehashi-download-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&root).unwrap(); Self(root)
        }
        fn model(&self) -> std::path::PathBuf { self.0.join("model.bin") }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for entry in std::fs::read_dir(&self.0).unwrap().flatten() {
                assert!(entry.path().is_file());
                std::fs::remove_file(entry.path()).unwrap();
            }
            std::fs::remove_dir(&self.0).unwrap();
        }
    }
    async fn server(responses: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 2048];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0); request.extend_from_slice(&buffer[..n]);
                }
                requests.push(String::from_utf8(request).unwrap().to_ascii_lowercase());
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.flush().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(15)).await;
                socket.shutdown().await.unwrap();
            }
            requests
        });
        (url, task)
    }
    fn hash(text: &str) -> String { format!("{:x}", Sha256::digest(text.as_bytes())) }

    #[tokio::test]
    async fn unavailable_model_fails_without_connection_retries_or_installing_an_error_page() {
        for status in ["404 Not Found", "403 Forbidden", "410 Gone"] {
            let fixture = Fixture::new();
            let (url, task) = server(vec![format!(
                "HTTP/1.1 {status}\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad"
            )]).await;
            let states = std::sync::Mutex::new(Vec::new());
            let error = download_file(&url, &fixture.model(), "", Arc::new(AtomicBool::new(false)),
                |_, _, state| states.lock().unwrap().push(state.to_owned())).await.unwrap_err();
            assert!(error.contains(status));
            assert_eq!(task.await.unwrap().len(), 1);
            assert!(!states.lock().unwrap().iter().any(|state| state == "retrying"));
            assert!(!fixture.model().exists());
            assert!(!fixture.model().with_extension("download").exists());
        }
    }

    #[tokio::test]
    async fn temporary_server_failure_retries_and_verifies_the_model() {
        let fixture = Fixture::new();
        let (url, task) = server(vec![
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nmodel".into(),
        ]).await;
        let states = std::sync::Mutex::new(Vec::new());
        download_file(&url, &fixture.model(), &hash("model"), Arc::new(AtomicBool::new(false)),
            |_, _, state| states.lock().unwrap().push(state.to_owned())).await.unwrap();
        assert_eq!(task.await.unwrap().len(), 2);
        assert!(states.lock().unwrap().iter().any(|state| state == "retrying"));
        assert_eq!(std::fs::read(fixture.model()).unwrap(), b"model");
    }

    #[tokio::test]
    async fn interrupted_body_is_resumed_and_verified_without_losing_existing_bytes() {
        let fixture = Fixture::new();
        let (url, task) = server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello".into(),
            "HTTP/1.1 206 Partial Content\r\nContent-Length: 6\r\nContent-Range: bytes 5-10/11\r\nConnection: close\r\n\r\n world".into(),
        ]).await;
        download_file(&url, &fixture.model(), &hash("hello world"), Arc::new(AtomicBool::new(false)), |_, _, _| {}).await.unwrap();
        assert_eq!(std::fs::read(fixture.model()).unwrap(), b"hello world");
        let requests = task.await.unwrap();
        assert!(requests[1].contains("range: bytes=5-"));
        assert!(!fixture.model().with_extension("download").exists());
    }

    #[tokio::test]
    async fn server_ignoring_range_restarts_instead_of_appending_the_full_file() {
        let fixture = Fixture::new();
        std::fs::write(fixture.model().with_extension("download"), b"old").unwrap();
        let (url, task) = server(vec!["HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nnew".into()]).await;
        download_file(&url, &fixture.model(), &hash("new"), Arc::new(AtomicBool::new(false)), |_, _, _| {}).await.unwrap();
        assert_eq!(std::fs::read(fixture.model()).unwrap(), b"new");
        assert!(task.await.unwrap()[0].contains("range: bytes=3-"));
    }

    #[tokio::test]
    async fn invalid_checksum_never_installs_a_corrupt_model_or_reuses_it_on_retry() {
        let fixture = Fixture::new();
        let (url, task) = server(vec!["HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".into()]).await;
        let error = download_file(&url, &fixture.model(), &hash("good"), Arc::new(AtomicBool::new(false)), |_, _, _| {}).await.unwrap_err();
        assert!(error.contains("checksum"));
        assert!(!fixture.model().exists());
        assert!(!fixture.model().with_extension("download").exists());
        task.await.unwrap();
    }

    #[tokio::test]
    async fn cancellation_during_a_stalled_body_flushes_and_keeps_received_bytes() {
        let fixture = Fixture::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            socket.read(&mut buffer).await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial").await.unwrap();
            socket.flush().await.unwrap();
            // End when the downloader closes the cancelled HTTP connection.
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buffer)).await;
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let task = {
            let cancel = cancel.clone(); let dest = fixture.model();
            tokio::spawn(async move { download_file(&url, &dest, &hash("unused"), cancel, |_, _, _| {}).await })
        };
        let partial = fixture.model().with_extension("download");
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0) < 7 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        cancel.store(true, Ordering::Release);
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), task).await.unwrap().unwrap();
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(std::fs::read(partial).unwrap(), b"partial");
        assert!(!fixture.model().exists());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_request_retains_a_partial_file_and_never_installs_it() {
        let fixture = Fixture::new();
        let partial = fixture.model().with_extension("download");
        std::fs::write(&partial, b"partial").unwrap();
        let result = download_file("http://127.0.0.1:1", &fixture.model(), "", Arc::new(AtomicBool::new(true)), |_, _, _| {}).await;
        assert!(result.unwrap_err().contains("cancelled"));
        assert_eq!(std::fs::read(partial).unwrap(), b"partial");
        assert!(!fixture.model().exists());
    }
}

/// Extract a .tar.bz2 archive into a destination directory.
/// Returns the path to the top-level extracted directory.
pub fn extract_tar_bz2(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    use bzip2::read::BzDecoder;
    use std::fs::File;

    log::info!(
        "Extracting archive: {} -> {}",
        archive_path.display(),
        dest_dir.display()
    );

    let file = File::open(archive_path)
        .map_err(|e| format!("Failed to open archive: {}", e))?;
    let decoder = BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    archive
        .unpack(dest_dir)
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    // Find the extracted top-level directory by scanning dest_dir.
    // tar.bz2 archives from sherpa-onnx always have a single top-level directory
    // whose name matches the model filename in the registry.
    let mut extracted_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dest_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    extracted_dirs.push(path);
                }
            }
        }
    }

    // If there's exactly one non-hidden directory, that's the extracted model dir
    if extracted_dirs.len() == 1 {
        log::info!("Extracted to: {}", extracted_dirs[0].display());
        return Ok(extracted_dirs[0].clone());
    }

    log::info!(
        "Extracted {} items to {}",
        extracted_dirs.len(),
        dest_dir.display()
    );
    Ok(dest_dir.to_path_buf())
}
