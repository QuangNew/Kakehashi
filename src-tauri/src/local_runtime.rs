//! Owns only the llama-server children started by this application.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

static SHARED_RUNTIME: OnceLock<Arc<LocalRuntime>> = OnceLock::new();
pub fn shared() -> Arc<LocalRuntime> {
    SHARED_RUNTIME
        .get_or_init(|| Arc::new(LocalRuntime::default()))
        .clone()
}
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Translation,
    Copilot,
    Embeddings,
}

impl ModelRole {
    pub const ALL: [Self; 3] = [Self::Translation, Self::Copilot, Self::Embeddings];
    fn index(self) -> usize {
        match self {
            Self::Translation => 0,
            Self::Copilot => 1,
            Self::Embeddings => 2,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Translation => "translation",
            Self::Copilot => "copilot",
            Self::Embeddings => "embeddings",
        }
    }
    pub fn endpoint(self) -> String {
        format!("http://127.0.0.1:{}/v1", 18181 + self.index())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSettings {
    pub file: String,
    pub context: u32,
    pub gpu_layers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LocalModelsConfig {
    pub models_dir: String,
    pub server_path: String,
    pub translation: ModelSettings,
    pub copilot: ModelSettings,
    pub embeddings: ModelSettings,
}

impl Default for LocalModelsConfig {
    fn default() -> Self {
        let executable_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let root = executable_dir
            .ancestors()
            .take(5)
            .find(|path| {
                path.join("models").is_dir()
                    && path.join("runtimes/llama/llama-server.exe").is_file()
            })
            .map(Path::to_path_buf)
            .unwrap_or_else(|| {
                if cfg!(debug_assertions) {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .to_path_buf()
                } else {
                    executable_dir.clone()
                }
            });
        Self {
            models_dir: root.join("models").to_string_lossy().into_owned(),
            server_path: root
                .join("runtimes/llama/llama-server.exe")
                .to_string_lossy()
                .into_owned(),
            translation: ModelSettings {
                file: "translation/Hy-MT2-7B-Q4_K_M.gguf".into(),
                context: 2048,
                gpu_layers: 22,
            },
            copilot: ModelSettings {
                file: "copilot/Qwen3-1.7B-Q4_K_M.gguf".into(),
                context: 4096,
                gpu_layers: 0,
            },
            embeddings: ModelSettings {
                file: "embeddings/Qwen3-Embedding-0.6B-Q8_0.gguf".into(),
                context: 2048,
                gpu_layers: 0,
            },
        }
    }
}

impl LocalModelsConfig {
    pub fn model(&self, role: ModelRole) -> &ModelSettings {
        match role {
            ModelRole::Translation => &self.translation,
            ModelRole::Copilot => &self.copilot,
            ModelRole::Embeddings => &self.embeddings,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !Path::new(&self.models_dir).is_absolute() || !Path::new(&self.server_path).is_absolute()
        {
            return Err("Choose an absolute models folder and llama-server path".into());
        }
        for role in ModelRole::ALL {
            let model = self.model(role);
            if !(512..=16384).contains(&model.context) || model.gpu_layers > 999 {
                return Err(format!(
                    "Invalid context or GPU layer count for {}",
                    role.name()
                ));
            }
        }
        Ok(())
    }
    fn model_path(&self, role: ModelRole) -> Result<PathBuf, String> {
        let name = &self.model(role).file;
        if name.trim().is_empty() {
            return Err(format!(
                "Choose a GGUF model for {} in Settings → Local Models",
                role.name()
            ));
        }
        let path = Path::new(&self.models_dir).join(name);
        if !path.is_file()
            || path
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
                != Some("gguf")
        {
            return Err(format!("GGUF model not found: {}", path.display()));
        }
        Ok(path)
    }
}

struct Server {
    child: Child,
    config: LocalModelsConfig,
    ready: bool,
    device: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeStatus {
    pub role: ModelRole,
    pub state: String,
    pub pid: Option<u32>,
    pub endpoint: String,
    pub error: Option<String>,
    pub device: Option<String>,
}

pub struct LocalRuntime {
    #[cfg(windows)]
    job: Option<std::os::windows::io::OwnedHandle>,
    config: RwLock<LocalModelsConfig>,
    config_path: RwLock<Option<PathBuf>>,
    lifecycle: Mutex<()>,
    servers: [Mutex<Option<Server>>; 3],
    generations: [AtomicU64; 3],
    errors: RwLock<[Option<String>; 3]>,
    shutdown: AtomicBool,
}

impl Default for LocalRuntime {
    fn default() -> Self {
        Self {
            #[cfg(windows)]
            job: create_process_job().ok(),
            config: RwLock::new(LocalModelsConfig::default()),
            config_path: RwLock::new(None),
            lifecycle: Mutex::new(()),
            servers: std::array::from_fn(|_| Mutex::new(None)),
            generations: std::array::from_fn(|_| AtomicU64::new(0)),
            errors: RwLock::new(std::array::from_fn(|_| None)),
            shutdown: AtomicBool::new(false),
        }
    }
}

impl LocalRuntime {
    /// Returns true only when a saved configuration was loaded.
    pub fn initialize(&self, path: PathBuf) -> Result<bool, String> {
        *self.config_path.write().map_err(|e| e.to_string())? = Some(path.clone());
        if !path.exists() {
            return Ok(false);
        }
        let config: LocalModelsConfig =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        config.validate()?;
        *self.config.write().map_err(|e| e.to_string())? = config;
        Ok(true)
    }
    pub fn config(&self) -> LocalModelsConfig {
        self.config.read().expect("local models config").clone()
    }

    pub async fn configure(&self, config: LocalModelsConfig) -> Result<(), String> {
        config.validate()?;
        if config == self.config()
            && self
                .config_path
                .read()
                .map_err(|e| e.to_string())?
                .as_ref()
                .map(|p| p.is_file())
                .unwrap_or(false)
        {
            return Ok(());
        }
        for generation in &self.generations {
            generation.fetch_add(1, Ordering::SeqCst);
        }
        let _lifecycle = self.lifecycle.lock().await;
        let path = self
            .config_path
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("Local model storage is not initialized")?;
        let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
        self.stop_all().await;
        *self.config.write().map_err(|e| e.to_string())? = config;
        Ok(())
    }

    pub async fn ensure(&self, role: ModelRole) -> Result<String, String> {
        let result = self.start_inner(role).await;
        if let Ok(mut errors) = self.errors.write() {
            errors[role.index()] = result.as_ref().err().cloned();
        }
        result
    }

    async fn start_inner(&self, role: ModelRole) -> Result<String, String> {
        let _lifecycle = self.lifecycle.lock().await;
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("Application is shutting down".into());
        }
        let config = self.config();
        let mut slot = self.servers[role.index()].lock().await;
        let generation = self.generations[role.index()].load(Ordering::SeqCst);
        if let Some(server) = slot.as_mut() {
            if server
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_none()
                && server.ready
                && server.config == config
            {
                return Ok(role.endpoint());
            }
            let _ = server.child.kill().await;
        }
        *slot = None;
        config.validate()?;
        let model_path = config.model_path(role)?;
        if !Path::new(&config.server_path).is_file() {
            return Err(
                "llama-server.exe not found. Choose the runtime in Settings → Local Models".into(),
            );
        }
        let port = 18181 + role.index();
        // Never adopt a server owned by another application.
        let probe = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port as u16))
            .map_err(|_| {
                format!(
                    "Port {} is already in use; stop the other server before starting {}",
                    port,
                    role.name()
                )
            })?;
        drop(probe);
        let settings = config.model(role);
        let device = if settings.gpu_layers > 0 {
            let mut probe = Command::new(&config.server_path);
            probe
                .arg("--list-devices")
                .stdin(Stdio::null())
                .kill_on_drop(true);
            #[cfg(windows)]
            probe.creation_flags(0x08000000);
            let output = tokio::time::timeout(Duration::from_secs(10), probe.output())
                .await
                .map_err(|_| "GPU detection timed out")?
                .map_err(|e| e.to_string())?;
            Some(
                preferred_gpu(&String::from_utf8_lossy(&output.stdout))
                    .ok_or("No GPU backend is available; select CPU in Local Models")?,
            )
        } else {
            None
        };
        let mut command = Command::new(&config.server_path);
        command
            .args(["--model"])
            .arg(model_path)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--alias",
                role.name(),
                "--ctx-size",
                &settings.context.to_string(),
                "--parallel",
                "1",
                "--n-gpu-layers",
                &settings.gpu_layers.to_string(),
            ])
            .current_dir(Path::new(&config.server_path).parent().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if role == ModelRole::Translation {
            command.args(["--threads","8","--threads-batch","8","--batch-size","256","--ubatch-size","128"]);
        }
        if role == ModelRole::Embeddings {
            command.args(["--embedding", "--pooling", "last"]);
        }
        if let Some(device) = &device {
            command.args(["--device", device]);
        }
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        if let Some(parent) = self
            .config_path
            .read()
            .map_err(|e| e.to_string())?
            .as_ref()
            .and_then(|p| p.parent())
        {
            if let Ok(log) =
                std::fs::File::create(parent.join(format!("llama-{}.log", role.name())))
            {
                command.stderr(log);
            }
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot launch llama-server: {}", e))?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::HANDLE, System::JobObjects::AssignProcessToJobObject,
            };
            let assignment = self
                .job
                .as_ref()
                .zip(child.raw_handle())
                .ok_or_else(|| "Cannot create the model process job".to_string())
                .and_then(|(job, process)| unsafe {
                    AssignProcessToJobObject(HANDLE(job.as_raw_handle()), HANDLE(process))
                        .map_err(|e| e.to_string())
                });
            if let Err(error) = assignment {
                let _ = child.kill().await;
                return Err(error);
            }
        }
        *slot = Some(Server {
            child,
            config,
            ready: false,
            device,
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        let health = format!("http://127.0.0.1:{}/health", port);
        let outcome = async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
            while tokio::time::Instant::now() < deadline {
                if self.shutdown.load(Ordering::SeqCst) || self.generations[role.index()].load(Ordering::SeqCst) != generation {
                    return Err("Model startup cancelled".to_string());
                }
                if let Some(status) = slot.as_mut().unwrap().child.try_wait().map_err(|e| e.to_string())? {
                    return Err(format!("llama-server exited ({}). Check llama-{}.log beside the local model configuration", status, role.name()));
                }
                if client.get(&health).send().await.map(|r| r.status().is_success()).unwrap_or(false) { slot.as_mut().unwrap().ready = true; return Ok(role.endpoint()); }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err("Model did not become ready within the startup timeout".into())
        }.await;
        if outcome.is_err() {
            if let Some(mut server) = slot.take() {
                let _ = server.child.kill().await;
            }
        }
        outcome
    }

    pub async fn stop(&self, role: ModelRole) {
        self.generations[role.index()].fetch_add(1, Ordering::SeqCst);
        if let Some(mut server) = self.servers[role.index()].lock().await.take() {
            let _ = server.child.kill().await;
        }
        if let Ok(mut errors) = self.errors.write() {
            errors[role.index()] = None;
        }
    }
    pub async fn stop_all(&self) {
        for generation in &self.generations {
            generation.fetch_add(1, Ordering::SeqCst);
        }
        for role in ModelRole::ALL {
            self.stop(role).await;
        }
    }
    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.stop_all().await;
    }
    pub fn status(&self) -> Vec<RuntimeStatus> {
        ModelRole::ALL
            .into_iter()
            .map(|role| {
                let mut status = RuntimeStatus {
                    role,
                    state: "stopped".into(),
                    pid: None,
                    endpoint: role.endpoint(),
                    error: self
                        .errors
                        .read()
                        .ok()
                        .and_then(|e| e[role.index()].clone()),
                    device: None,
                };
                match self.servers[role.index()].try_lock() {
                    Err(_) => status.state = "starting".into(),
                    Ok(mut slot) => {
                        if let Some(server) = slot.as_mut() {
                            status.device = server.device.clone();
                            match server.child.try_wait() {
                                Ok(None) => {
                                    status.state =
                                        if server.ready { "running" } else { "starting" }.into();
                                    status.pid = server.child.id();
                                }
                                _ => {
                                    status.state = "error".into();
                                    status.error = Some(
                                        "Model server has exited. Check the runtime log.".into(),
                                    );
                                    *slot = None;
                                }
                            }
                        }
                    }
                }
                if status.error.is_some() && status.state == "stopped" {
                    status.state = "error".into();
                }
                status
            })
            .collect()
    }
}

fn preferred_gpu(output: &str) -> Option<String> {
    let devices: Vec<_> = output
        .lines()
        .filter_map(|line| line.trim().split_once(':'))
        .filter(|(id, _)| {
            id.starts_with("Vulkan")
                || id.starts_with("CUDA")
                || id.starts_with("ROCm")
                || id.starts_with("SYCL")
        })
        .collect();
    devices
        .iter()
        .find(|(_, name)| name.contains("NVIDIA"))
        .or_else(|| devices.first())
        .map(|(id, _)| id.to_string())
}

#[cfg(windows)]
fn create_process_job() -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
    unsafe {
        let job = CreateJobObjectW(None, None).map_err(|e| e.to_string())?;
        let owned = OwnedHandle::from_raw_handle(job.0);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            HANDLE(owned.as_raw_handle()),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|e| e.to_string())?;
        Ok(owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_nvidia_before_integrated_gpu() {
        assert_eq!(
            preferred_gpu("Available devices:\n Vulkan0: Intel UHD\n Vulkan1: NVIDIA RTX 4050"),
            Some("Vulkan1".into())
        );
        assert_eq!(preferred_gpu("Available devices:"), None);
    }
    #[test]
    fn default_has_independent_roles_and_small_contexts() {
        let config = LocalModelsConfig::default();
        assert!(config.validate().is_ok());
        assert_ne!(
            ModelRole::Translation.endpoint(),
            ModelRole::Copilot.endpoint()
        );
        assert_eq!(config.copilot.gpu_layers, 0);
        assert!(config.copilot.file.ends_with(".gguf"));
    }
    #[test]
    fn rejects_invalid_limits_and_relative_roots() {
        let mut config = LocalModelsConfig::default();
        config.translation.context = 0;
        assert!(config.validate().is_err());
        config.translation.context = 2048;
        config.models_dir = "models".into();
        assert!(config.validate().is_err());
    }
}
