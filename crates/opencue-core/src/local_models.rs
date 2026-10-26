//! Speech models that run on this computer: speech-to-text (Parakeet, Canary, Whisper, SenseVoice)
//! and text-to-speech (Kokoro), downloaded from Settings → Models → Local.
//!
//! They run through the `sherpa-onnx` command-line tools (k2-fsa/sherpa-onnx): one small runtime
//! is downloaded once per computer, each model is a separate download, and nothing leaves the
//! machine. The same model files Lumina uses for meetings (Parakeet, Whisper, Kokoro) are offered
//! here, in the ONNX builds that sherpa-onnx reads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use crate::db::new_id;
use crate::events::CoreEvent;
use crate::Core;

/// The sherpa-onnx release the runtime comes from.
pub const SHERPA_VERSION: &str = "v2.0.0";
const SHERPA_BASE: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
pub const RUNTIME_ID: &str = "sherpa-runtime";
pub const DEFAULT_STT: &str = "parakeet-v3";
pub const DEFAULT_TTS: &str = "kokoro-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Stt,
    Tts,
    Runtime,
}

/// How a model is handed to the sherpa-onnx tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Transducer,
    Canary,
    Whisper,
    SenseVoice,
    KokoroMulti,
    KokoroEn,
}

struct Def {
    id: &'static str,
    kind: Kind,
    engine: Engine,
    label: &'static str,
    /// Download size in bytes (published asset sizes, shown before anything downloads).
    size: u64,
    license: &'static str,
    source: &'static str,
    asset: &'static str,
    /// Which release of k2-fsa/sherpa-onnx the asset is in.
    release: &'static str,
    languages: &'static [&'static str],
    note: &'static str,
    ram_gb: f32,
}

const MB: u64 = 1_000_000;

const EU25: &[&str] = &[
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt",
    "pl", "pt", "ro", "ru", "sk", "sl", "es", "sv", "uk",
];
const WHISPER_LANGS: &[&str] = &[
    "af", "ar", "bg", "bn", "ca", "cs", "da", "de", "el", "en", "es", "et", "eu", "fa", "fi", "fr",
    "gl", "he", "hi", "hr", "hu", "id", "is", "it", "ja", "ko", "lt", "lv", "ms", "nl", "no", "pl",
    "pt", "ro", "ru", "sk", "sl", "sr", "sv", "sw", "ta", "th", "tr", "uk", "ur", "vi", "zh",
];

const CATALOG: &[Def] = &[
    Def {
        id: "parakeet-v3",
        kind: Kind::Stt,
        engine: Engine::Transducer,
        label: "Parakeet TDT 0.6B v3 (8-bit)",
        size: 465 * MB,
        license: "CC-BY-4.0",
        source: "nvidia/parakeet-tdt-0.6b-v3 · sherpa-onnx build",
        asset: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
        release: "asr-models",
        languages: EU25,
        note: "Fast on any CPU. 25 European languages (Spanish and English included); it detects the language by itself.",
        ram_gb: 1.5,
    },
    Def {
        id: "canary-180m",
        kind: Kind::Stt,
        engine: Engine::Canary,
        label: "Canary 180M Flash (8-bit)",
        size: 147 * MB,
        license: "CC-BY-4.0",
        source: "nvidia/canary-180m-flash · sherpa-onnx build",
        asset: "sherpa-onnx-nemo-canary-180m-flash-en-es-de-fr-int8.tar.bz2",
        release: "asr-models",
        languages: &["en", "es", "de", "fr"],
        note: "Small and quick. English, Spanish, German and French; it follows the language you choose.",
        ram_gb: 0.8,
    },
    Def {
        id: "whisper-tiny",
        kind: Kind::Stt,
        engine: Engine::Whisper,
        label: "Whisper tiny",
        size: 111 * MB,
        license: "MIT",
        source: "openai/whisper-tiny · sherpa-onnx build",
        asset: "sherpa-onnx-whisper-tiny.tar.bz2",
        release: "asr-models",
        languages: WHISPER_LANGS,
        note: "99 languages on very small machines. The least accurate.",
        ram_gb: 0.4,
    },
    Def {
        id: "whisper-base",
        kind: Kind::Stt,
        engine: Engine::Whisper,
        label: "Whisper base",
        size: 198 * MB,
        license: "MIT",
        source: "openai/whisper-base · sherpa-onnx build",
        asset: "sherpa-onnx-whisper-base.tar.bz2",
        release: "asr-models",
        languages: WHISPER_LANGS,
        note: "99 languages, light.",
        ram_gb: 0.5,
    },
    Def {
        id: "whisper-small",
        kind: Kind::Stt,
        engine: Engine::Whisper,
        label: "Whisper small",
        size: 610 * MB,
        license: "MIT",
        source: "openai/whisper-small · sherpa-onnx build",
        asset: "sherpa-onnx-whisper-small.tar.bz2",
        release: "asr-models",
        languages: WHISPER_LANGS,
        note: "99 languages, a good balance.",
        ram_gb: 1.0,
    },
    Def {
        id: "whisper-turbo",
        kind: Kind::Stt,
        engine: Engine::Whisper,
        label: "Whisper large-v3 turbo",
        size: 538 * MB,
        license: "MIT",
        source: "openai/whisper-large-v3-turbo · sherpa-onnx build",
        asset: "sherpa-onnx-whisper-turbo.tar.bz2",
        release: "asr-models",
        languages: WHISPER_LANGS,
        note: "The most accurate Whisper here, for any language. Wants a recent CPU.",
        ram_gb: 3.0,
    },
    Def {
        id: "sensevoice",
        kind: Kind::Stt,
        engine: Engine::SenseVoice,
        label: "SenseVoice (8-bit)",
        size: 158 * MB,
        license: "FunASR Model License",
        source: "FunAudioLLM/SenseVoice · sherpa-onnx build",
        asset: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2",
        release: "asr-models",
        languages: &["zh", "en", "ja", "ko", "yue"],
        note: "Chinese, English, Japanese, Korean and Cantonese.",
        ram_gb: 0.8,
    },
    Def {
        id: "kokoro-v1",
        kind: Kind::Tts,
        engine: Engine::KokoroMulti,
        label: "Kokoro 82M v1.0 (multilingual)",
        size: 330 * MB,
        license: "Apache-2.0",
        source: "hexgrad/Kokoro-82M · sherpa-onnx build",
        asset: "kokoro-multi-lang-v1_0.tar.bz2",
        release: "tts-models",
        languages: &["en", "es", "fr", "it", "pt", "hi", "ja", "zh"],
        note: "Natural voices to read replies aloud: 54 voices in 8 languages, Spanish included.",
        ram_gb: 1.0,
    },
    Def {
        id: "kokoro-en",
        kind: Kind::Tts,
        engine: Engine::KokoroEn,
        label: "Kokoro 82M v0.19 (English, 8-bit)",
        size: 99 * MB,
        license: "Apache-2.0",
        source: "hexgrad/Kokoro-82M · sherpa-onnx build",
        asset: "kokoro-int8-en-v0_19.tar.bz2",
        release: "tts-models",
        languages: &["en"],
        note: "Small English-only voices for modest machines.",
        ram_gb: 0.5,
    },
];

/// The runtime asset for this computer, if sherpa-onnx ships one.
fn runtime_asset() -> Option<(String, u64)> {
    let v = SHERPA_VERSION;
    let (name, mb) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => (format!("sherpa-onnx-{v}-linux-x64-shared.tar.bz2"), 27),
        ("linux", "aarch64") => (
            format!("sherpa-onnx-{v}-linux-aarch64-shared-cpu.tar.bz2"),
            27,
        ),
        ("macos", _) => (format!("sherpa-onnx-{v}-osx-universal2-shared.tar.bz2"), 42),
        ("windows", "x86_64") => (
            format!("sherpa-onnx-{v}-win-x64-shared-MD-Release.tar.bz2"),
            21,
        ),
        _ => return None,
    };
    Some((name, mb * MB))
}

/// What the Models page shows for one downloadable item.
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub size: u64,
    pub license: String,
    pub source: String,
    pub languages: Vec<String>,
    pub note: String,
    pub ram_gb: f32,
    pub installed: bool,
    /// Whether this computer can run it (the runtime exists for its platform).
    pub supported: bool,
}

fn def(id: &str) -> Option<&'static Def> {
    CATALOG.iter().find(|d| d.id == id)
}

/// Kokoro v1.0 voice ids (the speaker numbers sherpa-onnx uses), by language.
pub const KOKORO_VOICES: &[(&str, &str, i32)] = &[
    ("en", "af_heart", 3),
    ("en", "af_bella", 2),
    ("en", "am_michael", 16),
    ("en", "bf_emma", 21),
    ("en", "bm_george", 26),
    ("es", "ef_dora", 28),
    ("es", "em_alex", 29),
    ("es", "em_santa", 53),
    ("fr", "ff_siwis", 30),
    ("it", "if_sara", 35),
    ("it", "im_nicola", 36),
    ("pt", "pf_dora", 42),
    ("pt", "pm_alex", 43),
    ("hi", "hf_alpha", 31),
    ("hi", "hm_omega", 33),
    ("ja", "jf_alpha", 37),
    ("zh", "zf_xiaobei", 45),
];

/// The speaker number for a voice name, or the first voice of a language.
pub fn kokoro_speaker(voice: Option<&str>, lang: &str) -> i32 {
    if let Some(v) = voice.filter(|v| !v.is_empty()) {
        if let Some((_, _, sid)) = KOKORO_VOICES.iter().find(|(_, n, _)| *n == v) {
            return *sid;
        }
    }
    KOKORO_VOICES
        .iter()
        .find(|(l, _, _)| *l == lang)
        .map(|(_, _, sid)| *sid)
        .unwrap_or(3)
}

// ---------- files ----------

fn find_file(dir: &Path, want: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| want(&n.to_ascii_lowercase()))
        })
        .collect();
    // The 8-bit file is the one to use when both exist.
    found.sort_by_key(|p| {
        !p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("int8"))
    });
    found.into_iter().next()
}

fn path_arg(flag: &str, p: &Path) -> String {
    format!("{flag}={}", p.display())
}

/// Arguments for `sherpa-onnx-offline` for one speech-to-text model (without the wav file).
fn stt_args(engine: Engine, dir: &Path, lang: Option<&str>) -> anyhow::Result<Vec<String>> {
    let need = |p: Option<PathBuf>, what: &str| {
        p.ok_or_else(|| {
            anyhow::anyhow!("the model folder is missing its {what}: download it again")
        })
    };
    let tokens = |dir: &Path| {
        need(
            find_file(dir, |n| n.ends_with("tokens.txt")),
            "tokens file (tokens.txt)",
        )
    };
    let mut a = vec!["--num-threads=4".to_string(), "--debug=0".to_string()];
    match engine {
        Engine::Transducer => {
            a.push(path_arg(
                "--encoder",
                &need(
                    find_file(dir, |n| n.contains("encoder") && n.ends_with(".onnx")),
                    "encoder",
                )?,
            ));
            a.push(path_arg(
                "--decoder",
                &need(
                    find_file(dir, |n| n.contains("decoder") && n.ends_with(".onnx")),
                    "decoder",
                )?,
            ));
            a.push(path_arg(
                "--joiner",
                &need(
                    find_file(dir, |n| n.contains("joiner") && n.ends_with(".onnx")),
                    "joiner",
                )?,
            ));
            a.push(path_arg("--tokens", &tokens(dir)?));
            a.push("--model-type=nemo_transducer".into());
        }
        Engine::Canary => {
            a.push(path_arg(
                "--canary-encoder",
                &need(
                    find_file(dir, |n| n.contains("encoder") && n.ends_with(".onnx")),
                    "encoder",
                )?,
            ));
            a.push(path_arg(
                "--canary-decoder",
                &need(
                    find_file(dir, |n| n.contains("decoder") && n.ends_with(".onnx")),
                    "decoder",
                )?,
            ));
            a.push(path_arg("--tokens", &tokens(dir)?));
            // Canary translates between its languages, so the source and the target are the same one.
            let l = lang
                .filter(|l| ["en", "es", "de", "fr"].contains(l))
                .unwrap_or("en");
            a.push(format!("--canary-src-lang={l}"));
            a.push(format!("--canary-tgt-lang={l}"));
        }
        Engine::Whisper => {
            a.push(path_arg(
                "--whisper-encoder",
                &need(
                    find_file(dir, |n| n.contains("encoder") && n.ends_with(".onnx")),
                    "encoder",
                )?,
            ));
            a.push(path_arg(
                "--whisper-decoder",
                &need(
                    find_file(dir, |n| n.contains("decoder") && n.ends_with(".onnx")),
                    "decoder",
                )?,
            ));
            a.push(path_arg("--tokens", &tokens(dir)?));
            a.push("--whisper-task=transcribe".into());
            if let Some(l) = lang {
                a.push(format!("--whisper-language={l}"));
            }
        }
        Engine::SenseVoice => {
            a.push(path_arg(
                "--sense-voice-model",
                &need(
                    find_file(dir, |n| n.ends_with(".onnx") && n.contains("model")),
                    "model",
                )?,
            ));
            a.push(path_arg("--tokens", &tokens(dir)?));
            a.push(format!("--sense-voice-language={}", lang.unwrap_or("auto")));
            a.push("--sense-voice-use-itn=1".into());
        }
        _ => anyhow::bail!("that model is not a speech-to-text model"),
    }
    Ok(a)
}

/// Arguments for `sherpa-onnx-offline-tts` (without the output file and the text).
fn tts_args(engine: Engine, dir: &Path, sid: i32) -> anyhow::Result<Vec<String>> {
    let need = |p: Option<PathBuf>, what: &str| {
        p.ok_or_else(|| {
            anyhow::anyhow!("the voice folder is missing its {what}: download it again")
        })
    };
    let mut a = vec![
        "--num-threads=4".to_string(),
        "--debug=0".to_string(),
        format!("--sid={sid}"),
        path_arg(
            "--kokoro-model",
            &need(
                find_file(dir, |n| n.starts_with("model") && n.ends_with(".onnx")),
                "model",
            )?,
        ),
        path_arg(
            "--kokoro-voices",
            &need(find_file(dir, |n| n == "voices.bin"), "voices file")?,
        ),
        path_arg(
            "--kokoro-tokens",
            &need(find_file(dir, |n| n == "tokens.txt"), "tokens file")?,
        ),
        path_arg("--kokoro-data-dir", &dir.join("espeak-ng-data")),
    ];
    if engine == Engine::KokoroMulti {
        a.push(path_arg("--kokoro-dict-dir", &dir.join("dict")));
        let lex: Vec<String> = ["lexicon-us-en.txt", "lexicon-zh.txt"]
            .iter()
            .map(|n| dir.join(n))
            .filter(|p| p.exists())
            .map(|p| p.display().to_string())
            .collect();
        if !lex.is_empty() {
            a.push(format!("--kokoro-lexicon={}", lex.join(",")));
        }
        let fsts: Vec<String> = ["phone-zh.fst", "date-zh.fst", "number-zh.fst"]
            .iter()
            .map(|n| dir.join(n))
            .filter(|p| p.exists())
            .map(|p| p.display().to_string())
            .collect();
        if !fsts.is_empty() {
            a.push(format!("--tts-rule-fsts={}", fsts.join(",")));
        }
    }
    Ok(a)
}

/// The text a `sherpa-onnx-offline` run printed: it ends with a JSON line `{"text": "…", …}`.
pub fn parse_transcript(stdout: &str) -> String {
    for line in stdout.lines().rev() {
        let line = line.trim();
        if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                    return t.trim().to_string();
                }
            }
        }
    }
    String::new()
}

/// Unpack a `.tar.bz2`, dropping its top folder (`name/file` → `file`). Refuses paths that climb out.
pub fn extract_tar_bz2(archive: &Path, dest: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(file));
    std::fs::create_dir_all(dest)?;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let mut rel = PathBuf::new();
        for (i, c) in path.components().enumerate() {
            match c {
                std::path::Component::Normal(p) => {
                    if i > 0 {
                        rel.push(p)
                    }
                }
                std::path::Component::CurDir => {}
                _ => anyhow::bail!("the archive has an unsafe path: {}", path.display()),
            }
        }
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&out)?;
    }
    Ok(())
}

/// Where the sherpa-onnx tools look for their shared libraries.
fn lib_env(runtime: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    let lib = runtime.join("lib");
    let bin = runtime.join("bin");
    let join = |var: &str, extra: &[&Path]| {
        let mut parts: Vec<PathBuf> = extra.iter().map(|p| p.to_path_buf()).collect();
        if let Some(cur) = std::env::var_os(var) {
            parts.extend(std::env::split_paths(&cur));
        }
        std::env::join_paths(parts).unwrap_or_default()
    };
    vec![
        ("LD_LIBRARY_PATH", join("LD_LIBRARY_PATH", &[&lib])),
        ("DYLD_LIBRARY_PATH", join("DYLD_LIBRARY_PATH", &[&lib])),
        ("PATH", join("PATH", &[&bin, &lib])),
    ]
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[derive(Default)]
pub(crate) struct Downloads {
    running: Mutex<HashMap<String, CancellationToken>>,
}

impl Downloads {
    fn start(&self, id: &str) -> Option<CancellationToken> {
        let mut m = self.running.lock().unwrap();
        if m.contains_key(id) {
            return None;
        }
        let t = CancellationToken::new();
        m.insert(id.to_string(), t.clone());
        Some(t)
    }

    fn finish(&self, id: &str) {
        self.running.lock().unwrap().remove(id);
    }

    fn cancel(&self, id: &str) {
        if let Some(t) = self.running.lock().unwrap().remove(id) {
            t.cancel();
        }
    }
}

impl Core {
    fn models_dir(&self) -> PathBuf {
        self.paths().root().join("models")
    }

    fn model_dir(&self, id: &str) -> PathBuf {
        self.models_dir().join(id)
    }

    fn model_installed(&self, id: &str) -> bool {
        self.model_dir(id).join(".complete").exists()
    }

    pub fn models_status(&self) -> Vec<ModelInfo> {
        let mut out: Vec<ModelInfo> = CATALOG
            .iter()
            .map(|d| ModelInfo {
                id: d.id.into(),
                kind: d.kind,
                label: d.label.into(),
                size: d.size,
                license: d.license.into(),
                source: d.source.into(),
                languages: d.languages.iter().map(|s| s.to_string()).collect(),
                note: d.note.into(),
                ram_gb: d.ram_gb,
                installed: self.model_installed(d.id),
                supported: runtime_asset().is_some(),
            })
            .collect();
        let asset = runtime_asset();
        out.push(ModelInfo {
            id: RUNTIME_ID.into(),
            kind: Kind::Runtime,
            label: format!("sherpa-onnx runtime ({SHERPA_VERSION})"),
            size: asset.as_ref().map(|a| a.1).unwrap_or(0),
            license: "Apache-2.0".into(),
            source: "github.com/k2-fsa/sherpa-onnx".into(),
            languages: vec![],
            note: "The small program that runs the models below. It downloads by itself with the first model.".into(),
            ram_gb: 0.0,
            installed: self.model_installed(RUNTIME_ID),
            supported: asset.is_some(),
        });
        out
    }

    fn sherpa_tool(&self, name: &str) -> anyhow::Result<PathBuf> {
        let dir = self.model_dir(RUNTIME_ID);
        anyhow::ensure!(
            self.model_installed(RUNTIME_ID),
            "The local speech runtime is not downloaded yet: Settings → Models → Local."
        );
        for p in [dir.join("bin").join(exe(name)), dir.join(exe(name))] {
            if p.exists() {
                return Ok(p);
            }
        }
        anyhow::bail!(
            "{name} is missing from the downloaded runtime: remove it in Settings → Models → Local and download it again."
        )
    }

    /// Download and unpack one item (the runtime first, when a model needs it).
    pub async fn model_install(self: &std::sync::Arc<Self>, id: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            id == RUNTIME_ID || def(id).is_some(),
            "unknown model `{id}`"
        );
        let token = self
            .downloads
            .start(id)
            .ok_or_else(|| anyhow::anyhow!("that download is already running"))?;
        let core = self.clone();
        let id = id.to_string();
        tokio::spawn(async move {
            let result = core.install_chain(&id, &token).await;
            core.downloads.finish(&id);
            let (phase, error) = match result {
                Ok(()) => ("ready", None),
                Err(_) if token.is_cancelled() => ("cancelled", None),
                Err(e) => ("error", Some(format!("{e:#}"))),
            };
            core.emit(CoreEvent::ModelProgress {
                id,
                phase: phase.into(),
                done: 0,
                total: 0,
                error,
            });
        });
        Ok(())
    }

    async fn install_chain(&self, id: &str, token: &CancellationToken) -> anyhow::Result<()> {
        if id != RUNTIME_ID && !self.model_installed(RUNTIME_ID) {
            self.install_one(RUNTIME_ID, token).await?;
        }
        self.install_one(id, token).await
    }

    async fn install_one(&self, id: &str, token: &CancellationToken) -> anyhow::Result<()> {
        let (url, label) = if id == RUNTIME_ID {
            let (asset, _) = runtime_asset().ok_or_else(|| {
                anyhow::anyhow!(
                    "Local speech models are not available for this system ({} {}).",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                )
            })?;
            (
                format!("{SHERPA_BASE}/{SHERPA_VERSION}/{asset}"),
                "runtime".to_string(),
            )
        } else {
            let d = def(id).ok_or_else(|| anyhow::anyhow!("unknown model `{id}`"))?;
            (
                format!("{SHERPA_BASE}/{}/{}", d.release, d.asset),
                d.label.to_string(),
            )
        };
        let tmp = self.models_dir().join(".tmp");
        std::fs::create_dir_all(&tmp)?;
        let part = tmp.join(format!("{id}.part"));
        let result = self.fetch_and_unpack(id, &url, &part, token).await;
        let _ = std::fs::remove_file(&part);
        result.map_err(|e| anyhow::anyhow!("{label}: {e:#}"))
    }

    async fn fetch_and_unpack(
        &self,
        id: &str,
        url: &str,
        part: &Path,
        token: &CancellationToken,
    ) -> anyhow::Result<()> {
        use futures::StreamExt;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        let res = http.get(url).send().await?;
        anyhow::ensure!(
            res.status().is_success(),
            "download failed ({}) from {url}",
            res.status()
        );
        let total = res.content_length().unwrap_or(0);
        let mut file = tokio::fs::File::create(part).await?;
        let mut stream = res.bytes_stream();
        let mut done = 0u64;
        let mut last = std::time::Instant::now() - Duration::from_secs(1);
        loop {
            let next = tokio::select! {
                n = stream.next() => n,
                _ = token.cancelled() => anyhow::bail!("cancelled"),
            };
            let Some(chunk) = next else { break };
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            done += chunk.len() as u64;
            if last.elapsed() >= Duration::from_millis(250) {
                last = std::time::Instant::now();
                self.emit(CoreEvent::ModelProgress {
                    id: id.into(),
                    phase: "downloading".into(),
                    done,
                    total,
                    error: None,
                });
            }
        }
        file.flush().await?;
        drop(file);
        self.emit(CoreEvent::ModelProgress {
            id: id.into(),
            phase: "extracting".into(),
            done,
            total,
            error: None,
        });
        let dest = self.model_dir(id);
        let _ = std::fs::remove_dir_all(&dest);
        let (archive, out) = (part.to_path_buf(), dest.clone());
        let unpacked = tokio::task::spawn_blocking(move || extract_tar_bz2(&archive, &out)).await?;
        if let Err(e) = unpacked {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(e);
        }
        std::fs::write(dest.join(".complete"), SHERPA_VERSION)?;
        Ok(())
    }

    pub fn model_cancel(&self, id: &str) {
        self.downloads.cancel(id);
    }

    pub fn model_delete(&self, id: &str) -> anyhow::Result<()> {
        self.downloads.cancel(id);
        let dir = self.model_dir(id);
        anyhow::ensure!(
            id == RUNTIME_ID || def(id).is_some(),
            "unknown model `{id}`"
        );
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    // ---------- using them ----------

    /// The speech-to-text model chosen in Settings (or the default), when it is ready to use.
    pub async fn local_stt_ready(&self) -> Option<&'static str> {
        let id = self
            .store
            .setting("local_stt_model")
            .await
            .ok()
            .flatten()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| DEFAULT_STT.into());
        let d = def(&id).filter(|d| d.kind == Kind::Stt)?;
        (self.model_installed(d.id) && self.model_installed(RUNTIME_ID)).then_some(d.id)
    }

    pub async fn local_tts_ready(&self) -> Option<&'static str> {
        let id = self
            .store
            .setting("local_tts_model")
            .await
            .ok()
            .flatten()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| DEFAULT_TTS.into());
        let d = def(&id).filter(|d| d.kind == Kind::Tts)?;
        (self.model_installed(d.id) && self.model_installed(RUNTIME_ID)).then_some(d.id)
    }

    async fn run_sherpa(
        &self,
        tool: &str,
        args: Vec<String>,
        extra: Option<&Path>,
    ) -> anyhow::Result<String> {
        let bin = self.sherpa_tool(tool)?;
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.args(&args);
        if let Some(e) = extra {
            cmd.arg(e);
        }
        for (k, v) in lib_env(&self.model_dir(RUNTIME_ID)) {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let out = tokio::time::timeout(Duration::from_secs(180), cmd.output())
            .await
            .map_err(|_| anyhow::anyhow!("{tool} took too long"))??;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let tail: String = err
                .lines()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!("{tool} failed: {}", tail.trim());
        }
        Ok(stdout)
    }

    /// WAV bytes (16-bit PCM) → text, with the speech model downloaded in Settings → Models.
    pub async fn local_transcribe(
        &self,
        wav: Vec<u8>,
        lang: Option<&str>,
    ) -> anyhow::Result<String> {
        let id = self.local_stt_ready().await.ok_or_else(|| {
            anyhow::anyhow!("Download a speech-to-text model in Settings → Models → Local first.")
        })?;
        let d = def(id).expect("catalog");
        let tmp = self.models_dir().join(".tmp");
        std::fs::create_dir_all(&tmp)?;
        let wav_path = tmp.join(format!("{}.wav", new_id()));
        std::fs::write(&wav_path, &wav)?;
        let lang = lang.map(|l| l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase());
        let args = stt_args(
            d.engine,
            &self.model_dir(id),
            lang.as_deref().filter(|l| !l.is_empty() && *l != "system"),
        );
        let result = match args {
            Ok(a) => {
                self.run_sherpa("sherpa-onnx-offline", a, Some(&wav_path))
                    .await
            }
            Err(e) => Err(e),
        };
        let _ = std::fs::remove_file(&wav_path);
        Ok(parse_transcript(&result?))
    }

    /// Text → WAV bytes with the voice model downloaded in Settings → Models.
    pub async fn local_speak(&self, text: &str, lang: &str) -> anyhow::Result<Vec<u8>> {
        let id = self.local_tts_ready().await.ok_or_else(|| {
            anyhow::anyhow!("Download a voice in Settings → Models → Local first.")
        })?;
        let d = def(id).expect("catalog");
        let voice = self.store.setting("local_tts_voice").await.ok().flatten();
        let sid = if d.engine == Engine::KokoroEn {
            0
        } else {
            kokoro_speaker(voice.as_deref(), lang)
        };
        let tmp = self.models_dir().join(".tmp");
        std::fs::create_dir_all(&tmp)?;
        let out_path = tmp.join(format!("{}.wav", new_id()));
        let mut args = tts_args(d.engine, &self.model_dir(id), sid)?;
        args.push(path_arg("--output-filename", &out_path));
        args.push(text.chars().take(2500).collect());
        let ran = self.run_sherpa("sherpa-onnx-offline-tts", args, None).await;
        let bytes = std::fs::read(&out_path);
        let _ = std::fs::remove_file(&out_path);
        ran?;
        bytes.map_err(|e| anyhow::anyhow!("the voice made no audio: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn catalog_ids_are_unique_and_sized() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|d| d.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        assert!(CATALOG
            .iter()
            .all(|d| d.size > 0 && d.asset.ends_with(".tar.bz2")));
        assert!(def(DEFAULT_STT).is_some_and(|d| d.kind == Kind::Stt));
        assert!(def(DEFAULT_TTS).is_some_and(|d| d.kind == Kind::Tts));
    }

    #[test]
    fn reads_the_text_from_sherpa_output() {
        let out = "Creating recognizer ...\n/tmp/a.wav\n{\"lang\": \"\", \"text\": \" hola mundo\", \"tokens\": []}\nDone!\n";
        assert_eq!(parse_transcript(out), "hola mundo");
        assert_eq!(parse_transcript("nothing here"), "");
    }

    #[test]
    fn speakers_follow_the_language() {
        assert_eq!(kokoro_speaker(None, "es"), 28);
        assert_eq!(kokoro_speaker(Some("em_santa"), "en"), 53);
        assert_eq!(kokoro_speaker(None, "xx"), 3);
    }

    #[test]
    fn builds_the_whisper_and_parakeet_commands() {
        let dir = tempfile::tempdir().unwrap();
        for f in [
            "tiny-encoder.onnx",
            "tiny-encoder.int8.onnx",
            "tiny-decoder.int8.onnx",
            "tiny-tokens.txt",
        ] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        let a = stt_args(Engine::Whisper, dir.path(), Some("es")).unwrap();
        assert!(a
            .iter()
            .any(|x| x.starts_with("--whisper-encoder=") && x.contains("int8")));
        assert!(a.contains(&"--whisper-language=es".to_string()));
        assert!(stt_args(Engine::Transducer, dir.path(), None).is_err());
        let c = stt_args(Engine::Canary, dir.path(), Some("pt")).unwrap();
        assert!(c.contains(&"--canary-src-lang=en".to_string()));
    }

    #[test]
    fn extracts_without_the_top_folder_and_refuses_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.tar.bz2");
        {
            let enc = bzip2::write::BzEncoder::new(
                std::fs::File::create(&archive).unwrap(),
                bzip2::Compression::fast(),
            );
            let mut b = tar::Builder::new(enc);
            let data = b"hello";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            b.append_data(&mut h, "top/bin/tool", &data[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap().flush().unwrap();
        }
        let out = dir.path().join("out");
        extract_tar_bz2(&archive, &out).unwrap();
        assert_eq!(std::fs::read(out.join("bin/tool")).unwrap(), b"hello");
    }
}
