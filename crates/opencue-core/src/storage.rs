//! What OpenCue keeps on disk, by kind, for Settings → Storage.

use std::path::Path;

use serde::Serialize;

/// One kind of data and how much room it takes.
#[derive(Debug, Clone, Serialize)]
pub struct StorageItem {
    /// `chats`, `agents`, `files`, `artifacts`, `models`, `memory`, `messenger`, `space` or `other`.
    pub id: String,
    pub bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageReport {
    pub root: String,
    pub total: u64,
    /// Largest first; kinds with nothing in them are left out.
    pub items: Vec<StorageItem>,
    /// Free and total space of the disk the data folder is on, when the system says.
    pub disk_free: Option<u64>,
    pub disk_total: Option<u64>,
}

/// Which kind a top-level entry of the data folder belongs to.
fn kind_of(name: &str) -> &'static str {
    match name {
        n if n.starts_with("opencue.db") => "chats",
        "agents" => "agents",
        "files" | "shared" => "files",
        "artifacts" | "dashboard" => "artifacts",
        "models" => "models",
        "memory" | "skills" => "memory",
        "messenger" => "messenger",
        "space" => "space",
        _ => "other",
    }
}

/// Size and file count of a file or folder. Links are counted, not followed.
fn measure(path: &Path) -> (u64, u64) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return (0, 0);
    };
    if !meta.is_dir() {
        return (meta.len(), 1);
    }
    let (mut bytes, mut files) = (0, 0);
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                bytes += meta.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}

/// Free and total bytes of the disk holding `path` (from `df`; not on Windows).
fn disk_space(path: &Path) -> (Option<u64>, Option<u64>) {
    #[cfg(unix)]
    {
        let out = std::process::Command::new("df")
            .arg("-Pk")
            .arg(path)
            .stderr(std::process::Stdio::null())
            .output();
        if let Ok(out) = out {
            let text = String::from_utf8_lossy(&out.stdout);
            // Filesystem 1024-blocks Used Available Capacity Mounted-on
            if let Some(line) = text.lines().nth(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 4 {
                    let kb = |s: &str| s.parse::<u64>().ok().map(|v| v * 1024);
                    return (kb(cols[3]), kb(cols[1]));
                }
            }
        }
    }
    let _ = path;
    (None, None)
}

/// Walk the data folder and add it up by kind. It reads the whole tree, so call it off the async
/// runtime (see [`crate::Core::storage_report`]).
pub fn report(root: &Path) -> StorageReport {
    let mut sums: std::collections::BTreeMap<&'static str, (u64, u64)> = Default::default();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let (bytes, files) = measure(&entry.path());
            let sum = sums.entry(kind_of(&name)).or_default();
            sum.0 += bytes;
            sum.1 += files;
        }
    }
    let mut items: Vec<StorageItem> = sums
        .into_iter()
        .filter(|(_, (bytes, _))| *bytes > 0)
        .map(|(id, (bytes, files))| StorageItem {
            id: id.to_string(),
            bytes,
            files,
        })
        .collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let (disk_free, disk_total) = disk_space(root);
    StorageReport {
        root: root.display().to_string(),
        total: items.iter().map(|i| i.bytes).sum(),
        items,
        disk_free,
        disk_total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_up_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("opencue.db"), vec![0u8; 100]).unwrap();
        std::fs::write(root.join("opencue.db-wal"), vec![0u8; 20]).unwrap();
        std::fs::create_dir_all(root.join("agents/a1/workspace")).unwrap();
        std::fs::write(root.join("agents/a1/workspace/x.txt"), vec![0u8; 300]).unwrap();
        std::fs::write(root.join("agents/a1/MEMORY.md"), vec![0u8; 5]).unwrap();
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::write(root.join("secrets.json"), vec![0u8; 7]).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();

        let r = report(root);
        assert_eq!(r.total, 432);
        assert_eq!(r.items[0].id, "agents");
        assert_eq!(r.items[0].bytes, 305);
        assert_eq!(r.items[0].files, 2);
        let chats = r.items.iter().find(|i| i.id == "chats").unwrap();
        assert_eq!((chats.bytes, chats.files), (120, 2));
        assert!(r.items.iter().any(|i| i.id == "other" && i.bytes == 7));
        // Kinds with nothing in them are left out.
        assert!(!r.items.iter().any(|i| i.id == "memory"));
    }
}
