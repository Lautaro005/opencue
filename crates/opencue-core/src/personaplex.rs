//! NVIDIA PersonaPlex (https://github.com/NVIDIA/personaplex): a full-duplex speech-to-speech model
//! (7B, built on Moshi) that talks back in real time, without turn-taking. OpenCue installs its
//! Python server in a private virtual environment, starts and stops it, and shows where to open it,
//! on this computer or from a phone on the same network (with a QR code).
//!
//! It needs an NVIDIA GPU (or `--cpu-offload`, which is slow), Python 3.10+, the Opus library, and a
//! Hugging Face token whose owner accepted the model license: the weights download the first time
//! the server starts.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::events::CoreEvent;
use crate::Core;

pub const PORT: u16 = 8998;
pub const HF_TOKEN_KEY: &str = "huggingface";
const ARCHIVE: &str = "https://github.com/NVIDIA/personaplex/archive/refs/heads/main.zip#subdirectory=moshi";
const LOG_LINES: usize = 400;

#[derive(Default)]
pub(crate) struct Runtime {
    child: Mutex<Option<Child>>,
    log: Mutex<Vec<String>>,
    installing: std::sync::atomic::AtomicBool,
}

impl Runtime {
    fn push(&self, line: impl Into<String>) {
        let mut log = self.log.lock().unwrap();
        log.push(line.into());
        let extra = log.len().saturating_sub(LOG_LINES);
        log.drain(..extra);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaStatus {
    pub installed: bool,
    pub installing: bool,
    pub running: bool,
    /// The web page answers.
    pub ready: bool,
    pub has_token: bool,
    pub lan: bool,
    pub offload: bool,
    pub gpu: Option<String>,
    pub python: Option<String>,
    pub url: Option<String>,
    /// Address for a phone on the same network (when LAN access is on).
    pub lan_url: Option<String>,
    pub qr_svg: Option<String>,
    pub log: String,
}

fn venv_python(dir: &std::path::Path) -> PathBuf {
    if cfg!(windows) {
        dir.join("venv").join("Scripts").join("python.exe")
    } else {
        dir.join("venv").join("bin").join("python")
    }
}

/// This computer's address on the local network (what a phone would type).
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packet is sent: connecting a UDP socket only picks the outgoing interface.
    s.connect("192.0.2.1:80").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

fn qr_svg(text: &str) -> Option<String> {
    let qr = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        qr.render::<qrcode::render::svg::Color>()
            .min_dimensions(220, 220)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

/// The `python -m moshi.server` arguments.
pub fn server_args(ssl_dir: &std::path::Path, lan: bool, offload: bool) -> Vec<String> {
    let mut a = vec![
        "-m".into(),
        "moshi.server".into(),
        "--host".into(),
        if lan { "0.0.0.0" } else { "localhost" }.into(),
        "--port".into(),
        PORT.to_string(),
        "--ssl".into(),
        ssl_dir.display().to_string(),
    ];
    if offload {
        a.push("--cpu-offload".into());
    }
    a
}

async fn capture(cmd: &mut Command, secs: u64) -> Option<String> {
    cmd.stdin(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = tokio::time::timeout(Duration::from_secs(secs), cmd.output())
        .await
        .ok()?
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A Python 3.10+ on this computer: its command and version.
async fn find_python() -> Option<(String, String)> {
    for name in ["python3", "python", "py"] {
        let mut cmd = Command::new(name);
        if name == "py" {
            cmd.arg("-3");
        }
        cmd.args([
            "-c",
            "import sys;print('%d.%d' % sys.version_info[:2] if sys.version_info>=(3,10) else '')",
        ]);
        if let Some(v) = capture(&mut cmd, 8).await.filter(|v| !v.is_empty()) {
            return Some((name.to_string(), v));
        }
    }
    None
}

async fn gpu_name() -> Option<String> {
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(["--query-gpu=name,memory.total", "--format=csv,noheader"]);
    capture(&mut cmd, 4)
        .await
        .map(|s| s.lines().next().unwrap_or_default().to_string())
        .filter(|s| !s.is_empty())
}

async fn port_open() -> bool {
    tokio::time::timeout(
        Duration::from_millis(400),
        tokio::net::TcpStream::connect(("127.0.0.1", PORT)),
    )
    .await
    .is_ok_and(|r| r.is_ok())
}

impl Core {
    fn persona_dir(&self) -> PathBuf {
        self.paths().root().join("models").join("personaplex")
    }

    fn persona_installed(&self) -> bool {
        self.persona_dir().join(".complete").exists()
    }

    pub async fn personaplex_status(&self) -> PersonaStatus {
        let rt = &self.persona;
        let running = {
            let mut child = rt.child.lock().unwrap();
            match child.as_mut().map(|c| c.try_wait()) {
                Some(Ok(None)) => true,
                Some(_) => {
                    *child = None;
                    false
                }
                None => false,
            }
        };
        let lan = self.setting_flag("personaplex_lan").await;
        let offload = self.setting_flag("personaplex_offload").await;
        let ready = running && port_open().await;
        let lan_url = lan_ip().map(|ip| format!("https://{ip}:{PORT}"));
        PersonaStatus {
            installed: self.persona_installed(),
            installing: rt.installing.load(std::sync::atomic::Ordering::SeqCst),
            running,
            ready,
            has_token: self
                .secrets
                .get(HF_TOKEN_KEY)
                .is_some_and(|k| !k.is_empty()),
            lan,
            offload,
            gpu: gpu_name().await,
            python: find_python().await.map(|(_, v)| v),
            url: running.then(|| format!("https://localhost:{PORT}")),
            qr_svg: if lan && running {
                lan_url.as_deref().and_then(qr_svg)
            } else {
                None
            },
            lan_url: if lan { lan_url } else { None },
            log: rt.log.lock().unwrap().join("\n"),
        }
    }

    async fn setting_flag(&self, key: &str) -> bool {
        self.store.setting(key).await.ok().flatten().as_deref() == Some("true")
    }

    /// Save the options and the Hugging Face token (the token is kept in the keychain).
    pub async fn personaplex_configure(
        &self,
        lan: bool,
        offload: bool,
        token: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(t) = token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            anyhow::ensure!(
                t.starts_with("hf_"),
                "a Hugging Face token starts with hf_ (huggingface.co/settings/tokens)"
            );
            self.secrets.set(HF_TOKEN_KEY, &t)?;
        }
        self.store
            .set_setting("personaplex_lan", if lan { "true" } else { "false" })
            .await?;
        self.store
            .set_setting("personaplex_offload", if offload { "true" } else { "false" })
            .await?;
        Ok(())
    }

    /// Pipe a child's output into the log, line by line.
    fn persona_pipe(self: &Arc<Self>, child: &mut Child) {
        if let Some(out) = child.stdout.take() {
            let core = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    core.persona.push(l);
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let core = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    core.persona.push(l);
                }
            });
        }
    }

    async fn persona_run(
        self: &Arc<Self>,
        program: &str,
        args: &[&str],
    ) -> anyhow::Result<()> {
        self.persona.push(format!("$ {program} {}", args.join(" ")));
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let mut child = cmd.spawn()?;
        self.persona_pipe(&mut child);
        let status = child.wait().await?;
        anyhow::ensure!(status.success(), "`{program}` failed ({status}); see the log");
        Ok(())
    }

    /// Create the virtual environment and install PersonaPlex's server in it.
    pub async fn personaplex_install(self: &Arc<Self>) -> anyhow::Result<()> {
        use std::sync::atomic::Ordering;
        anyhow::ensure!(
            !self.persona.installing.swap(true, Ordering::SeqCst),
            "the installation is already running"
        );
        let (python, _) = match find_python().await {
            Some(p) => p,
            None => {
                self.persona.installing.store(false, Ordering::SeqCst);
                anyhow::bail!("PersonaPlex needs Python 3.10 or newer: install it from python.org first.")
            }
        };
        let core = self.clone();
        tokio::spawn(async move {
            let dir = core.persona_dir();
            let result: anyhow::Result<()> = async {
                std::fs::create_dir_all(&dir)?;
                let _ = std::fs::remove_file(dir.join(".complete"));
                let mut base: Vec<&str> = vec![];
                if python == "py" {
                    base.push("-3");
                }
                let venv = dir.join("venv");
                if !venv_python(&dir).exists() {
                    let venv_s = venv.display().to_string();
                    let mut a = base.clone();
                    a.extend(["-m", "venv", &venv_s]);
                    core.persona_run(&python, &a).await?;
                }
                let py = venv_python(&dir).display().to_string();
                core.persona_run(&py, &["-m", "pip", "install", "--upgrade", "pip"])
                    .await?;
                core.persona_run(&py, &["-m", "pip", "install", ARCHIVE, "accelerate"])
                    .await?;
                std::fs::write(dir.join(".complete"), "ok")?;
                Ok(())
            }
            .await;
            core.persona.installing.store(false, Ordering::SeqCst);
            let error = result.err().map(|e| {
                core.persona.push(format!("error: {e:#}"));
                format!("{e:#}")
            });
            core.emit(CoreEvent::ModelProgress {
                id: "personaplex".into(),
                phase: if error.is_some() { "error" } else { "ready" }.into(),
                done: 0,
                total: 0,
                error,
            });
        });
        Ok(())
    }

    pub async fn personaplex_start(self: &Arc<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.persona_installed(),
            "Install PersonaPlex first (Settings → Models → Local)."
        );
        let token = self
            .secrets
            .get(HF_TOKEN_KEY)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Add your Hugging Face token first, and accept the license at huggingface.co/nvidia/personaplex-7b-v1."
                )
            })?;
        {
            let mut child = self.persona.child.lock().unwrap();
            if let Some(c) = child.as_mut() {
                if matches!(c.try_wait(), Ok(None)) {
                    return Ok(());
                }
            }
            *child = None;
        }
        let dir = self.persona_dir();
        let ssl = dir.join("ssl");
        std::fs::create_dir_all(&ssl)?;
        let lan = self.setting_flag("personaplex_lan").await;
        let offload = self.setting_flag("personaplex_offload").await;
        let mut cmd = Command::new(venv_python(&dir));
        cmd.args(server_args(&ssl, lan, offload))
            .env("HF_TOKEN", token)
            .env("PYTHONUNBUFFERED", "1")
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        self.persona.push("— starting PersonaPlex (the first start downloads the model) —");
        let mut child = cmd.spawn()?;
        self.persona_pipe(&mut child);
        *self.persona.child.lock().unwrap() = Some(child);
        Ok(())
    }

    pub async fn personaplex_stop(&self) {
        let child = self.persona.child.lock().unwrap().take();
        if let Some(mut c) = child {
            let _ = c.start_kill();
            let _ = c.wait().await;
            self.persona.push("— PersonaPlex stopped —");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_listens_where_asked() {
        let ssl = std::path::Path::new("/tmp/ssl");
        let local = server_args(ssl, false, false);
        assert!(local.windows(2).any(|w| w == ["--host", "localhost"]));
        assert!(!local.contains(&"--cpu-offload".to_string()));
        let phone = server_args(ssl, true, true);
        assert!(phone.windows(2).any(|w| w == ["--host", "0.0.0.0"]));
        assert!(phone.contains(&"--cpu-offload".to_string()));
        assert!(phone.windows(2).any(|w| w == ["--port", "8998"]));
    }

    #[test]
    fn makes_a_qr_code() {
        assert!(qr_svg("https://192.168.1.5:8998")
            .unwrap()
            .starts_with("<?xml"));
    }
}
