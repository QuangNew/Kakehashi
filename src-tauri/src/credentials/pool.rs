//! API Pool metadata contains no secrets; key material stays in the OS vault.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub trait Vault {
    fn read(&self, name: &str) -> Result<Option<String>, String>;
    fn write(&self, name: &str, value: &str) -> Result<(), String>;
    fn delete(&self, name: &str) -> Result<(), String>;
}
pub struct WindowsVault;
impl Vault for WindowsVault {
    fn read(&self, name: &str) -> Result<Option<String>, String> {
        super::windows_cred::credential_read(name)
    }
    fn write(&self, name: &str, value: &str) -> Result<(), String> {
        super::windows_cred::credential_write(name, value)
    }
    fn delete(&self, name: &str) -> Result<(), String> {
        super::windows_cred::credential_delete(name)
    }
}

pub const PROVIDERS: &[&str] = &[
    "openai",
    "anthropic",
    "groq",
    "gemini",
    "openrouter",
    "deepgram",
    "azure_speech",
    "microsoft",
    "google_translation",
    "deepl",
    "custom",
];

#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub region: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Data {
    pub entries: Vec<Entry>,
    pub defaults: BTreeMap<String, String>,
    pub bindings: BTreeMap<String, String>,
    #[serde(default)]
    legacy_imported: bool,
}
#[derive(Serialize)]
pub struct EntryStatus {
    #[serde(flatten)]
    pub entry: Entry,
    pub key_present: bool,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub entries: Vec<EntryStatus>,
    pub defaults: BTreeMap<String, String>,
    pub bindings: BTreeMap<String, String>,
}
#[derive(Deserialize)]
pub struct SaveEntry {
    pub id: Option<String>,
    pub provider: String,
    pub label: String,
    pub key: Option<String>,
    pub region: Option<String>,
}
pub struct ApiPool {
    path: PathBuf,
    data: Data,
}

/// Explicit bindings win. Groq/OpenAI can be shared across all their capabilities.
pub fn route(request: &str) -> Option<(String, String, bool)> {
    let request = request.trim().to_ascii_lowercase();
    let is_region = request.ends_with("_region");
    let name = request.strip_suffix("_region").unwrap_or(&request);
    let (usage, provider) = match name {
        "whisper_api" => ("stt", "openai"),
        "groq_whisper" => ("stt", "groq"),
        "deepgram" | "azure_speech" => ("stt", name),
        name if name.starts_with("translation_") => {
            ("translation", name.trim_start_matches("translation_"))
        }
        name => ("copilot", name),
    };
    let provider = if provider == "google" {
        "google_translation"
    } else {
        provider
    };
    if !PROVIDERS.contains(&provider) {
        return None;
    }
    Some((provider.into(), format!("{usage}.{provider}"), is_region))
}

impl ApiPool {
    pub fn load(path: PathBuf, vault: &dyn Vault) -> Result<Self, String> {
        let data = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "API Pool metadata is invalid; the original file was retained")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(error) => return Err(error.to_string()),
        };
        let mut pool = Self { path, data };
        if !pool.data.legacy_imported {
            for name in [
                "openai",
                "whisper_api",
                "anthropic",
                "groq",
                "groq_whisper",
                "translation_groq",
                "gemini",
                "translation_gemini",
                "openrouter",
                "deepgram",
                "azure_speech",
                "translation_microsoft",
                "translation_google",
                "translation_deepl",
                "custom",
            ] {
                let Some(key) = vault.read(name)?.filter(|k| !k.trim().is_empty()) else {
                    continue;
                };
                let (provider, binding, _) = route(name).ok_or("Invalid legacy provider")?;
                let region = if provider == "azure_speech" {
                    vault.read("azure_speech_region")?
                } else if provider == "microsoft" {
                    vault.read("translation_microsoft_region")?
                } else {
                    None
                };
                let mut existing = None;
                for entry in &pool.data.entries {
                    if entry.provider == provider
                        && entry.region == region
                        && vault.read(&format!("api_pool:{}", entry.id))?.as_deref()
                            == Some(key.as_str())
                    {
                        existing = Some(entry.id.clone());
                        break;
                    }
                }
                let id = match existing {
                    Some(id) => id,
                    None => pool.save(
                        SaveEntry {
                            id: None,
                            provider,
                            label: format!("Đã lưu · {name}"),
                            key: Some(key),
                            region,
                        },
                        vault,
                    )?,
                };
                pool.data.bindings.insert(binding, id);
            }
            pool.data.legacy_imported = true;
            pool.persist()?;
        }
        Ok(pool)
    }
    fn persist(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temporary = self.path.with_extension("json.pending");
        let bytes = serde_json::to_vec_pretty(&self.data).map_err(|e| e.to_string())?;
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }
    pub fn snapshot(&self, vault: &dyn Vault) -> Result<Snapshot, String> {
        let mut entries = Vec::new();
        for entry in &self.data.entries {
            entries.push(EntryStatus {
                entry: entry.clone(),
                key_present: vault
                    .read(&format!("api_pool:{}", entry.id))?
                    .is_some_and(|key| !key.trim().is_empty()),
            });
        }
        Ok(Snapshot {
            entries,
            defaults: self.data.defaults.clone(),
            bindings: self.data.bindings.clone(),
        })
    }
    pub fn resolve(&self, request: &str, vault: &dyn Vault) -> Result<Option<String>, String> {
        let Some((provider, binding, region)) = route(request) else {
            return vault.read(request);
        };
        let id = self
            .data
            .bindings
            .get(&binding)
            .or_else(|| self.data.defaults.get(&provider));
        let entry = id.and_then(|id| {
            self.data
                .entries
                .iter()
                .find(|entry| &entry.id == id && entry.provider == provider)
        });
        let Some(entry) = entry else {
            return Ok(None);
        };
        if region {
            return Ok(entry.region.clone());
        }
        vault.read(&format!("api_pool:{}", entry.id))
    }
    /// Keep older IPC clients consistent with the pool after migration.
    pub fn store_request(
        &mut self,
        request: &str,
        value: &str,
        vault: &dyn Vault,
    ) -> Result<(), String> {
        let Some((provider, binding, is_region)) = route(request) else {
            return vault.write(request, value);
        };
        let selected = self
            .data
            .bindings
            .get(&binding)
            .or_else(|| self.data.defaults.get(&provider));
        let entry = selected
            .and_then(|id| {
                self.data
                    .entries
                    .iter()
                    .find(|e| &e.id == id && e.provider == provider)
            })
            .cloned();
        if is_region && entry.is_none() {
            return Err("Add this provider's key to API Pool before setting its region".into());
        }
        let id = self.save(
            SaveEntry {
                id: entry.as_ref().map(|e| e.id.clone()),
                provider,
                label: entry
                    .as_ref()
                    .map(|e| e.label.clone())
                    .unwrap_or_else(|| format!("Đã lưu · {request}")),
                key: if is_region { None } else { Some(value.into()) },
                region: if is_region {
                    Some(value.into())
                } else {
                    entry.and_then(|e| e.region)
                },
            },
            vault,
        )?;
        let previous = self.data.clone();
        self.data.bindings.insert(binding, id);
        if let Err(error) = self.persist() {
            self.data = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn clear_request(&mut self, request: &str, vault: &dyn Vault) -> Result<(), String> {
        let Some((_, binding, _)) = route(request) else {
            return vault.delete(request);
        };
        let previous = self.data.clone();
        self.data.bindings.insert(binding, String::new());
        if let Err(error) = self.persist() {
            self.data = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn save(&mut self, input: SaveEntry, vault: &dyn Vault) -> Result<String, String> {
        if !PROVIDERS.contains(&input.provider.as_str())
            || input.label.trim().is_empty()
            || input.label.chars().count() > 80
        {
            return Err("Choose a supported provider and a label of 1–80 characters".into());
        }
        let previous = self.data.clone();
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let old = self.data.entries.iter().find(|entry| entry.id == id);
        if input.id.is_some() && old.is_none() {
            return Err("API Pool entry no longer exists".into());
        }
        if old.is_some_and(|entry| entry.provider != input.provider) {
            return Err("Create a new entry to change provider".into());
        }
        let target = format!("api_pool:{id}");
        let old_key = vault.read(&target)?;
        let new_key = input
            .key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty());
        if new_key.is_none() && old_key.is_none() {
            return Err("Enter an API key".into());
        }
        if new_key.is_some_and(|key| key.len() > 4096 || key.contains(['\r', '\n'])) {
            return Err("API key is invalid".into());
        }
        let entry = Entry {
            id: id.clone(),
            provider: input.provider.clone(),
            label: input.label.trim().into(),
            region: input
                .region
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().into()),
        };
        self.data.entries.retain(|entry| entry.id != id);
        self.data.entries.push(entry);
        self.data
            .defaults
            .entry(input.provider)
            .or_insert(id.clone());
        if let Some(key) = new_key {
            if let Err(error) = vault.write(&target, key) {
                self.data = previous;
                return Err(error);
            }
        }
        if let Err(error) = self.persist() {
            self.data = previous;
            if let Some(key) = old_key {
                let _ = vault.write(&target, &key);
            } else {
                let _ = vault.delete(&target);
            }
            return Err(error);
        }
        Ok(id)
    }
    pub fn bind(&mut self, usage: &str, provider: &str, id: Option<String>) -> Result<(), String> {
        if !["copilot", "translation", "stt"].contains(&usage) || !PROVIDERS.contains(&provider) {
            return Err("Invalid API Pool binding".into());
        }
        if let Some(id) = &id {
            if !self
                .data
                .entries
                .iter()
                .any(|entry| &entry.id == id && entry.provider == provider)
            {
                return Err("The selected key belongs to a different provider".into());
            }
        }
        let old = self.data.clone();
        let binding = format!("{usage}.{provider}");
        if let Some(id) = id {
            self.data.bindings.insert(binding, id);
        } else {
            self.data.bindings.remove(&binding);
        }
        if let Err(error) = self.persist() {
            self.data = old;
            return Err(error);
        }
        Ok(())
    }
    pub fn delete(&mut self, id: &str, vault: &dyn Vault) -> Result<(), String> {
        if !self.data.entries.iter().any(|entry| entry.id == id) {
            return Err("API Pool entry no longer exists".into());
        }
        let previous = self.data.clone();
        self.data.entries.retain(|entry| entry.id != id);
        // Retain an explicit empty binding: never silently switch accounts or
        // resurrect an archived legacy key after the user deletes a selection.
        for value in self
            .data
            .bindings
            .values_mut()
            .chain(self.data.defaults.values_mut())
        {
            if value == id {
                value.clear();
            }
        }
        if let Err(error) = self.persist() {
            self.data = previous;
            return Err(error);
        }
        vault.delete(&format!("api_pool:{id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Default)]
    struct Memory(RefCell<BTreeMap<String, String>>);
    impl Vault for Memory {
        fn read(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self.0.borrow().get(name).cloned())
        }
        fn write(&self, name: &str, value: &str) -> Result<(), String> {
            self.0.borrow_mut().insert(name.into(), value.into());
            Ok(())
        }
        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.borrow_mut().remove(name);
            Ok(())
        }
    }
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!("kakehashi-pool-{}.json", uuid::Uuid::new_v4()))
    }
    #[test]
    fn shared_key_covers_translation_copilot_and_stt_and_can_be_overridden() {
        let path = path();
        let vault = Memory::default();
        let mut pool = ApiPool::load(path.clone(), &vault).unwrap();
        let first = pool
            .save(
                SaveEntry {
                    id: None,
                    provider: "groq".into(),
                    label: "Personal".into(),
                    key: Some("key-one".into()),
                    region: None,
                },
                &vault,
            )
            .unwrap();
        for request in ["groq", "translation_groq", "groq_whisper"] {
            assert_eq!(
                pool.resolve(request, &vault).unwrap().as_deref(),
                Some("key-one")
            );
        }
        let second = pool
            .save(
                SaveEntry {
                    id: None,
                    provider: "groq".into(),
                    label: "Work".into(),
                    key: Some("key-two".into()),
                    region: None,
                },
                &vault,
            )
            .unwrap();
        pool.bind("translation", "groq", Some(second.clone()))
            .unwrap();
        assert_eq!(
            pool.resolve("translation_groq", &vault).unwrap().as_deref(),
            Some("key-two")
        );
        assert_eq!(
            pool.resolve("groq", &vault).unwrap().as_deref(),
            Some("key-one")
        );
        assert!(pool.bind("copilot", "gemini", Some(first)).is_err());
        let reloaded = ApiPool::load(path.clone(), &vault).unwrap();
        assert_eq!(
            reloaded
                .resolve("translation_groq", &vault)
                .unwrap()
                .as_deref(),
            Some("key-two")
        );
        assert!(!std::fs::read_to_string(&path).unwrap().contains("key-two"));
        let snapshot = serde_json::to_string(&pool.snapshot(&vault).unwrap()).unwrap();
        assert!(!snapshot.contains("key-one"));
        pool.delete(&second, &vault).unwrap();
        assert!(pool.resolve("translation_groq", &vault).unwrap().is_none());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn imports_legacy_keys_once_without_overwriting_separate_translation_accounts() {
        let path = path();
        let vault = Memory::default();
        vault.write("groq", "copilot-key").unwrap();
        vault.write("translation_groq", "translation-key").unwrap();
        let mut pool = ApiPool::load(path.clone(), &vault).unwrap();
        assert_eq!(pool.snapshot(&vault).unwrap().entries.len(), 2);
        assert_eq!(
            pool.resolve("groq", &vault).unwrap().as_deref(),
            Some("copilot-key")
        );
        assert_eq!(
            pool.resolve("translation_groq", &vault).unwrap().as_deref(),
            Some("translation-key")
        );
        let id = pool.data.bindings["translation.groq"].clone();
        pool.delete(&id, &vault).unwrap();
        let pool = ApiPool::load(path.clone(), &vault).unwrap();
        assert!(pool.resolve("translation_groq", &vault).unwrap().is_none());
        assert_eq!(
            vault.read("translation_groq").unwrap().as_deref(),
            Some("translation-key")
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn older_clients_update_and_clear_the_selected_binding_after_import() {
        let path = path();
        let vault = Memory::default();
        let mut pool = ApiPool::load(path.clone(), &vault).unwrap();
        pool.store_request("groq", "first", &vault).unwrap();
        assert_eq!(
            pool.resolve("groq_whisper", &vault).unwrap().as_deref(),
            Some("first")
        );
        pool.store_request("groq", "updated", &vault).unwrap();
        assert_eq!(
            pool.resolve("groq", &vault).unwrap().as_deref(),
            Some("updated")
        );
        pool.clear_request("translation_groq", &vault).unwrap();
        assert!(pool.resolve("translation_groq", &vault).unwrap().is_none());
        assert_eq!(
            pool.resolve("groq", &vault).unwrap().as_deref(),
            Some("updated")
        );
        std::fs::remove_file(path).unwrap();
    }
}
