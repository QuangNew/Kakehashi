pub mod windows_cred;
pub mod pool;
use std::sync::Mutex;

/// Credential manager for secure API key storage.
/// Delegates to the platform-specific credential backend (Windows Credential Manager).
pub struct CredentialManager {
    pool: Option<Mutex<pool::ApiPool>>,
}

impl CredentialManager {
    pub fn new() -> Self {
        Self { pool: None }
    }

    pub fn initialize_pool(&mut self, path: std::path::PathBuf) -> Result<(), String> {
        self.pool = Some(Mutex::new(pool::ApiPool::load(path, &pool::WindowsVault)?));
        Ok(())
    }
    pub fn pool_snapshot(&self) -> Result<pool::Snapshot, String> {
        self.pool.as_ref().ok_or("API Pool is not initialized")?.lock().map_err(|e|e.to_string())?.snapshot(&pool::WindowsVault)
    }
    pub fn pool_save(&self, input: pool::SaveEntry) -> Result<String, String> {
        self.pool.as_ref().ok_or("API Pool is not initialized")?.lock().map_err(|e|e.to_string())?.save(input,&pool::WindowsVault)
    }
    pub fn pool_bind(&self, usage: &str, provider: &str, id: Option<String>) -> Result<(), String> {
        self.pool.as_ref().ok_or("API Pool is not initialized")?.lock().map_err(|e|e.to_string())?.bind(usage,provider,id)
    }
    pub fn pool_delete(&self, id: &str) -> Result<(), String> {
        self.pool.as_ref().ok_or("API Pool is not initialized")?.lock().map_err(|e|e.to_string())?.delete(id,&pool::WindowsVault)
    }

    /// Store an API key for the given provider.
    pub fn store_key(&self, provider: &str, key: &str) -> Result<(), String> {
        let provider = Self::normalize_provider(provider);
        if let Some(pool)=&self.pool {return pool.lock().map_err(|e|e.to_string())?.store_request(&provider,key,&pool::WindowsVault);}
        windows_cred::credential_write(&provider, key)
    }

    /// Retrieve an API key for the given provider. Returns None if not stored.
    pub fn get_key(&self, provider: &str) -> Result<Option<String>, String> {
        let provider = Self::normalize_provider(provider);
        if let Some(pool) = &self.pool {
            return pool.lock().map_err(|e|e.to_string())?.resolve(&provider,&pool::WindowsVault);
        }
        windows_cred::credential_read(&provider)
    }

    /// Delete the API key for the given provider.
    pub fn delete_key(&self, provider: &str) -> Result<(), String> {
        let provider = Self::normalize_provider(provider);
        if let Some(pool)=&self.pool {return pool.lock().map_err(|e|e.to_string())?.clear_request(&provider,&pool::WindowsVault);}
        windows_cred::credential_delete(&provider)
    }

    /// Check whether an API key exists for the given provider.
    pub fn has_key(&self, provider: &str) -> Result<bool, String> {
        let provider = Self::normalize_provider(provider);
        let key = self.get_key(&provider)?;
        Ok(key.is_some_and(|key| !key.trim().is_empty()))
    }

    /// Normalize provider name to lowercase for consistent key naming.
    fn normalize_provider(provider: &str) -> String {
        provider.to_lowercase().trim().to_string()
    }
}
