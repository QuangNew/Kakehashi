//! Bounded on-disk logs survive a desktop launch without an attached terminal.
use std::io::{self, Write};
use std::path::PathBuf;

struct DesktopLog {
    file: Option<std::fs::File>,
    path: PathBuf,
    bytes: u64,
}

impl Write for DesktopLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Keep at most two 2 MiB log files; failure to write logs is non-fatal.
        if self.bytes + bytes.len() as u64 > 2 * 1024 * 1024 {
            self.file.take();
            let previous = self.path.with_extension("previous.log");
            let _ = std::fs::remove_file(&previous);
            let _ = std::fs::rename(&self.path, previous);
            self.file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path).ok();
            self.bytes = 0;
        }
        if let Some(file) = self.file.as_mut() {
            let _ = file.write_all(bytes);
            let _ = file.flush();
        }
        self.bytes += bytes.len() as u64;
        let _ = io::stderr().write_all(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub fn initialize() {
    let root = std::env::var_os("APPDATA").map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir).join("com.tvnjp.app").join("logs");
    let _ = std::fs::create_dir_all(&root);
    let path = root.join("app.log");
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(DesktopLog { file, path, bytes })))
        .try_init();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("Rust panic: {info}\n{}", std::backtrace::Backtrace::force_capture());
        previous(info);
    }));
    log::info!("Kakehashi starting; pid={}, version={}", std::process::id(), env!("CARGO_PKG_VERSION"));
}
