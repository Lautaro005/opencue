//! API keys live in the OS credential store (Keychain / Credential Manager / Secret Service).
//! On systems without one (headless Linux, minimal WMs) we fall back to a 0600 JSON file and say so.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

const SERVICE: &str = "opencue";

pub struct Secrets {
    fallback: PathBuf,
    lock: Mutex<()>,
}

impl Secrets {
    pub fn new(fallback: PathBuf) -> Self {
        Self {
            fallback,
            lock: Mutex::new(()),
        }
    }

    pub fn set(&self, name: &str, value: &str) -> anyhow::Result<()> {
        if value.is_empty() {
            return self.delete(name);
        }
        match keyring::Entry::new(SERVICE, name).and_then(|e| e.set_password(value)) {
            Ok(()) => {
                // Drop any stale fallback copy.
                let _ = self.file_remove(name);
                Ok(())
            }
            Err(err) => {
                tracing::warn!(
                    "keyring unavailable ({err}); storing secret in {}",
                    self.fallback.display()
                );
                self.file_set(name, value)
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<String> {
        if let Ok(v) = keyring::Entry::new(SERVICE, name).and_then(|e| e.get_password()) {
            return Some(v);
        }
        self.file_read().ok()?.remove(name)
    }

    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        if let Ok(entry) = keyring::Entry::new(SERVICE, name) {
            let _ = entry.delete_credential();
        }
        self.file_remove(name)
    }

    fn file_read(&self) -> anyhow::Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.fallback) {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn file_write(&self, map: &BTreeMap<String, String>) -> anyhow::Result<()> {
        std::fs::write(&self.fallback, serde_json::to_string_pretty(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.fallback, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    fn file_set(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.file_read()?;
        map.insert(name.to_string(), value.to_string());
        self.file_write(&map)
    }

    fn file_remove(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.file_read()?;
        if map.remove(name).is_some() {
            self.file_write(&map)?;
        }
        Ok(())
    }
}

pub fn provider_key_name(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}
