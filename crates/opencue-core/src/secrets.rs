//! API keys and tokens live in a `0600` JSON file (`secrets.json`) inside OpenCue's data folder.
//! They are not kept in the system keychain: it asked for permission over and over.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Secrets {
    file: PathBuf,
    lock: Mutex<()>,
}

impl Secrets {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            lock: Mutex::new(()),
        }
    }

    pub fn set(&self, name: &str, value: &str) -> anyhow::Result<()> {
        if value.is_empty() {
            return self.delete(name);
        }
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read()?;
        map.insert(name.to_string(), value.to_string());
        self.write(&map)
    }

    pub fn get(&self, name: &str) -> Option<String> {
        self.read().ok()?.remove(name)
    }

    pub fn delete(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut map = self.read()?;
        if map.remove(name).is_some() {
            self.write(&map)?;
        }
        Ok(())
    }

    fn read(&self) -> anyhow::Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.file) {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn write(&self, map: &BTreeMap<String, String>) -> anyhow::Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.file, serde_json::to_string_pretty(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.file, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

pub fn provider_key_name(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_secrets_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let s = Secrets::new(dir.path().join("secrets.json"));
        assert_eq!(s.get("a"), None);
        s.set("a", "1").unwrap();
        s.set("b", "2").unwrap();
        assert_eq!(s.get("a").as_deref(), Some("1"));
        s.delete("a").unwrap();
        assert_eq!(s.get("a"), None);
        assert_eq!(s.get("b").as_deref(), Some("2"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("secrets.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
