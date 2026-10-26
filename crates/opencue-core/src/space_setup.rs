//! Perplexity SPACE, installed and connected by OpenCue (Linux x86_64 with KVM only).
//!
//! SPACE (github.com/perplexityai/SPACE) is a Firecracker microVM platform that runs in a local Kind
//! cluster. Following its own installation guide, this module checks the host, clones the
//! repository, runs the prerequisite and Firecracker installers and the bring-up (`up.sh`), waits
//! for the API, signs in as `root@local` with a password it makes up, and fills in Settings →
//! Computers (address, template and token). The token is renewed from the saved password.
//!
//! Everything runs as the user; installing the prerequisites needs root or passwordless `sudo`, and
//! when that is missing the checks say so before anything starts.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

use crate::events::CoreEvent;
use crate::Core;

pub const API: &str = "http://127.0.0.1:8080";
const CLUSTER: &str = "space-local";
const PASSWORD_KEY: &str = "space_password";
const LOG_LINES: usize = 500;
/// A token is renewed after this long (SPACE sessions are not documented to live forever).
const TOKEN_MAX_AGE_SECS: i64 = 6 * 3600;
const REPO: &str = "https://github.com/perplexityai/SPACE.git";
const TOOLS: [&str; 6] = ["git", "curl", "tar", "openssl", "python3", "ssh-keygen"];

#[derive(Default)]
pub(crate) struct Setup {
    running: AtomicBool,
    state: Mutex<(String, String)>,
    log: Mutex<Vec<String>>,
    child: Mutex<Option<Child>>,
}

impl Setup {
    fn push(&self, line: impl Into<String>) {
        let mut log = self.log.lock().unwrap();
        log.push(line.into());
        let extra = log.len().saturating_sub(LOG_LINES);
        log.drain(..extra);
    }

    fn set(&self, state: &str, step: &str) {
        *self.state.lock().unwrap() = (state.to_string(), step.to_string());
    }
}

/// What the host looks like, gathered before anything is installed.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub os: String,
    pub arch: String,
    pub kvm: bool,
    pub tun: bool,
    pub docker: bool,
    pub apt: bool,
    pub root_or_sudo: bool,
    pub missing_tools: Vec<String>,
    pub free_gib: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Check {
    pub id: &'static str,
    pub ok: bool,
    pub detail: String,
}

/// One row per requirement, in the order the guide lists them.
pub fn evaluate(f: &Facts) -> Vec<Check> {
    let row = |id: &'static str, ok: bool, good: &str, bad: String| Check {
        id,
        ok,
        detail: if ok { good.to_string() } else { bad },
    };
    vec![
        row(
            "platform",
            f.os == "linux" && f.arch == "x86_64",
            "Linux x86_64",
            format!("{} {}: SPACE needs Linux on x86_64", f.os, f.arch),
        ),
        row(
            "kvm",
            f.kvm,
            "/dev/kvm is readable and writable",
            "/dev/kvm is missing or not accessible: enable virtualization (nested virtualization in a VM) and add your user to the kvm group".into(),
        ),
        row("tun", f.tun, "/dev/net/tun exists", "/dev/net/tun is missing".into()),
        row(
            "docker",
            f.docker,
            "Docker is running",
            "Docker is not running or not installed: SPACE builds inside Docker containers".into(),
        ),
        row(
            "apt",
            f.apt,
            "apt-based system",
            "SPACE's installer expects an apt-based Linux (Debian, Ubuntu)".into(),
        ),
        row(
            "tools",
            f.missing_tools.is_empty(),
            "git, curl, tar, openssl, python3 and ssh-keygen found",
            format!("missing: {}", f.missing_tools.join(", ")),
        ),
        row(
            "root",
            f.root_or_sudo,
            "root or passwordless sudo",
            "the installer needs root or passwordless sudo (run `sudo -v` in a terminal, or set it up)".into(),
        ),
        row(
            "disk",
            f.free_gib >= 30.0,
            "30 GiB or more free",
            format!("only {:.0} GiB free; SPACE needs at least 30 GiB", f.free_gib),
        ),
    ]
}

pub fn blockers(checks: &[Check]) -> Vec<&Check> {
    checks.iter().filter(|c| !c.ok).collect()
}

async fn ok(program: &str, args: &[&str]) -> bool {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(15), cmd.status())
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some_and(|s| s.success())
}

pub async fn gather_facts() -> Facts {
    let mut f = Facts {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        ..Default::default()
    };
    #[cfg(unix)]
    {
        f.kvm = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .is_ok();
        f.tun = Path::new("/dev/net/tun").exists();
        f.apt = which::which("apt-get").is_ok();
        f.docker = which::which("docker").is_ok()
            && ok("docker", &["version", "--format", "{{.Server.Version}}"]).await;
        f.root_or_sudo = ok("sh", &["-c", "[ \"$(id -u)\" = 0 ]"]).await
            || (which::which("sudo").is_ok() && ok("sudo", &["-n", "true"]).await);
        f.missing_tools = TOOLS
            .iter()
            .filter(|t| which::which(t).is_err())
            .map(|t| t.to_string())
            .collect();
        if let Some(home) = directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
            let out = Command::new("df")
                .args(["-Pk"])
                .arg(&home)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .await
                .ok();
            f.free_gib = out
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .nth(1)?
                        .split_whitespace()
                        .nth(3)?
                        .parse::<f64>()
                        .ok()
                })
                .map(|kb| kb / 1024.0 / 1024.0)
                .unwrap_or(0.0);
        }
    }
    f
}

/// A password for the local `root@local` account.
fn make_password() -> String {
    format!("{}{}", crate::db::new_id(), crate::db::new_id())
        .chars()
        .take(32)
        .collect()
}

/// Sign in to SPACE as `root@local` and return the bearer token.
pub async fn login(api: &str, password: &str) -> anyhow::Result<String> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .no_proxy()
        .build()?;
    let res = http
        .post(format!("{}/auth/v1/sessions", api.trim_end_matches('/')))
        .json(&json!({ "email": "root@local", "password": password }))
        .send()
        .await?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    anyhow::ensure!(
        status.is_success(),
        "SPACE did not accept the sign-in ({status})"
    );
    body.get("token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("SPACE answered without a token"))
}

async fn api_up(api: &str) -> bool {
    let Ok(http) = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .no_proxy()
        .build()
    else {
        return false;
    };
    http.get(format!("{}/openapi-m1.json", api.trim_end_matches('/')))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

#[derive(Debug, Clone, Serialize)]
pub struct SpaceStatus {
    /// Linux x86_64: the only place SPACE can run.
    pub supported: bool,
    /// idle, checking, installing, ready or error.
    pub state: String,
    pub step: String,
    pub checks: Vec<Check>,
    pub log: String,
    pub dir: String,
    /// OpenCue installed it, so it can renew the token.
    pub managed: bool,
    pub api_up: bool,
}

fn script_env(root: &Path) -> Vec<(&'static str, String)> {
    let home = directories::UserDirs::new()
        .map(|d| d.home_dir().display().to_string())
        .unwrap_or_default();
    let bin = format!("{home}/.local/bin");
    vec![
        ("INSTALL_DIR", bin.clone()),
        (
            "PATH",
            format!("{bin}:{}", std::env::var("PATH").unwrap_or_default()),
        ),
        (
            "SPACE_VENDOR_ROOT",
            format!("{home}/.local/share/space/vendor"),
        ),
        ("SPACE_KIND_CLUSTER", CLUSTER.into()),
        ("SPACE_SETUP_ROOT", root.display().to_string()),
    ]
}

impl Core {
    fn space_root(&self) -> PathBuf {
        self.paths().root().join("space")
    }

    pub async fn space_status(&self) -> SpaceStatus {
        let supported = cfg!(all(target_os = "linux", target_arch = "x86_64"));
        let (state, step) = self.space.state.lock().unwrap().clone();
        let running = self.space.running.load(Ordering::SeqCst);
        let managed = self.secrets.get(PASSWORD_KEY).is_some_and(|p| !p.is_empty());
        let checks = if supported && !running {
            evaluate(&gather_facts().await)
        } else {
            vec![]
        };
        let log = self.space.log.lock().unwrap().join("\n");
        let api_up = supported && api_up(API).await;
        SpaceStatus {
            supported,
            state: if state.is_empty() {
                if managed { "ready".into() } else { "idle".into() }
            } else {
                state
            },
            step,
            checks,
            log,
            dir: self.space_root().display().to_string(),
            managed,
            api_up,
        }
    }

    async fn space_step(
        self: &Arc<Self>,
        step: &str,
        script: &str,
        extra_env: &[(&str, String)],
    ) -> anyhow::Result<()> {
        self.space.set("installing", step);
        self.space.push(format!("— {step} —"));
        let root = self.space_root();
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(script)
            .current_dir(&root)
            .env("SPACE_SETUP_ROOT", root.display().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in script_env(&root) {
            cmd.env(k, v);
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.env_remove("AWS_ACCESS_KEY_ID")
            .env_remove("AWS_SECRET_ACCESS_KEY")
            .env_remove("AWS_SESSION_TOKEN");
        let mut child = cmd.spawn()?;
        for pipe in [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let core = self.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    core.space.push(l);
                }
            });
        }
        *self.space.child.lock().unwrap() = Some(child);
        // Wait without holding the lock: cancelling takes the child out and kills it.
        let status = loop {
            let done = {
                let mut guard = self.space.child.lock().unwrap();
                match guard.as_mut() {
                    Some(c) => c.try_wait()?,
                    None => anyhow::bail!("cancelled"),
                }
            };
            if let Some(s) = done {
                break s;
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        };
        *self.space.child.lock().unwrap() = None;
        anyhow::ensure!(status.success(), "{step} failed ({status}); see the log");
        Ok(())
    }

    /// Install SPACE and connect it. Runs in the background; poll [`Core::space_status`].
    pub async fn space_install(self: &Arc<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            cfg!(all(target_os = "linux", target_arch = "x86_64")),
            "SPACE runs on Linux x86_64 only: on this system, connect to a SPACE server on another computer."
        );
        anyhow::ensure!(
            !self.space.running.swap(true, Ordering::SeqCst),
            "the installation is already running"
        );
        self.space.set("checking", "checks");
        let checks = evaluate(&gather_facts().await);
        let bad = blockers(&checks);
        if !bad.is_empty() {
            self.space.running.store(false, Ordering::SeqCst);
            self.space.set("error", "checks");
            let msg = bad
                .iter()
                .map(|c| format!("• {}", c.detail))
                .collect::<Vec<_>>()
                .join("\n");
            self.space.push(format!("The computer is not ready:\n{msg}"));
            anyhow::bail!("This computer is not ready for SPACE:\n{msg}");
        }
        let core = self.clone();
        tokio::spawn(async move {
            let result = core.space_pipeline().await;
            core.space.running.store(false, Ordering::SeqCst);
            let error = match result {
                Ok(()) => {
                    core.space.set("ready", "done");
                    None
                }
                Err(e) => {
                    core.space.set("error", "failed");
                    core.space.push(format!("error: {e:#}"));
                    Some(format!("{e:#}"))
                }
            };
            core.emit(CoreEvent::ModelProgress {
                id: "space".into(),
                phase: if error.is_some() { "error" } else { "ready" }.into(),
                done: 0,
                total: 0,
                error,
            });
        });
        Ok(())
    }

    async fn space_pipeline(self: &Arc<Self>) -> anyhow::Result<()> {
        std::fs::create_dir_all(self.space_root())?;
        self.space_step(
            "Downloading SPACE",
            &format!(
                "set -euo pipefail; if [ -d SPACE/.git ]; then git -C SPACE pull --ff-only; else git clone --depth 1 {REPO} SPACE; fi"
            ),
            &[],
        )
        .await?;
        self.space_step(
            "Installing the tools (Kind, kubectl, Helm, Tilt, Go…)",
            "set -euo pipefail; mkdir -p \"$INSTALL_DIR\"; cd SPACE; bash infra/space/kind/scripts/install-prereqs.sh; \
             for t in kind kubectl helm grpcurl tilt go jq; do command -v $t >/dev/null || { echo \"missing after install: $t\"; exit 1; }; done",
            &[],
        )
        .await?;
        self.space_step(
            "Installing Firecracker",
            "set -euo pipefail; cd SPACE; bash infra/space/kind/scripts/install-fc-vendor.sh",
            &[],
        )
        .await?;
        // The root@local password: kept so the token can be renewed. A rerun keeps the old one,
        // because the bring-up only sets the password while the account is still locked.
        let password = match self.secrets.get(PASSWORD_KEY).filter(|p| !p.is_empty()) {
            Some(p) => p,
            None => {
                let p = make_password();
                self.secrets.set(PASSWORD_KEY, &p)?;
                p
            }
        };
        self.space_step(
            "Building and starting SPACE (the first time takes a long while)",
            "set -euo pipefail; cd SPACE; bash infra/space/kind/scripts/up.sh",
            &[("SPACE_CONSOLE_PASSWORD", password.clone())],
        )
        .await?;
        self.space.set("installing", "Checking the API");
        self.space.push("— waiting for the API —");
        let mut up = false;
        for _ in 0..120 {
            if api_up(API).await {
                up = true;
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        anyhow::ensure!(up, "the SPACE API did not answer on {API}");
        self.space.set("installing", "Connecting OpenCue");
        let token = login(API, &password).await?;
        self.space_connect(&token).await?;
        self.space.push("SPACE is ready: agents can now use it as their computer.");
        Ok(())
    }

    /// Fill in Settings → Computers for the local SPACE.
    async fn space_connect(&self, token: &str) -> anyhow::Result<()> {
        self.store.set_setting("space_url", API).await?;
        self.store.set_setting("space_template", "seed:latest").await?;
        self.secrets.set(crate::services::SPACE_KEY, token)?;
        self.store
            .set_setting("space_token_at", &chrono::Utc::now().timestamp().to_string())
            .await?;
        Ok(())
    }

    /// Start the stack again (after a reboot): the bring-up is safe to repeat.
    pub async fn space_start(self: &Arc<Self>) -> anyhow::Result<()> {
        let password = self
            .secrets
            .get(PASSWORD_KEY)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| anyhow::anyhow!("SPACE was not installed by OpenCue: use Install first."))?;
        anyhow::ensure!(
            !self.space.running.swap(true, Ordering::SeqCst),
            "something is already running"
        );
        let core = self.clone();
        tokio::spawn(async move {
            let r = async {
                core.space_step(
                    "Starting SPACE",
                    "set -euo pipefail; cd SPACE; bash infra/space/kind/scripts/up.sh",
                    &[("SPACE_CONSOLE_PASSWORD", password.clone())],
                )
                .await?;
                let token = login(API, &password).await?;
                core.space_connect(&token).await
            }
            .await;
            core.space.running.store(false, Ordering::SeqCst);
            match r {
                Ok(()) => core.space.set("ready", "done"),
                Err(e) => {
                    core.space.set("error", "failed");
                    core.space.push(format!("error: {e:#}"));
                }
            }
        });
        Ok(())
    }

    /// Remove the cluster (images and Firecracker files stay, as SPACE's own teardown leaves them).
    pub async fn space_stop(self: &Arc<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.space.running.swap(true, Ordering::SeqCst),
            "something is already running"
        );
        let r = self
            .space_step(
                "Stopping SPACE",
                "set -euo pipefail; cd SPACE; bash infra/space/kind/scripts/teardown.sh",
                &[],
            )
            .await;
        self.space.running.store(false, Ordering::SeqCst);
        self.space.set(if r.is_ok() { "idle" } else { "error" }, "");
        r
    }

    /// Cancel an installation in progress.
    pub fn space_cancel(&self) {
        if let Some(mut c) = self.space.child.lock().unwrap().take() {
            let _ = c.start_kill();
        }
    }

    /// Renew the token of a SPACE OpenCue installed, when it is old. Best effort, quick.
    pub(crate) async fn space_refresh_token(&self) {
        let Some(password) = self.secrets.get(PASSWORD_KEY).filter(|p| !p.is_empty()) else {
            return;
        };
        let url = self.store.setting("space_url").await.ok().flatten().unwrap_or_default();
        if url.trim_end_matches('/') != API {
            return;
        }
        let at: i64 = self
            .store
            .setting("space_token_at")
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let fresh = self.secrets.get(crate::services::SPACE_KEY).is_some_and(|t| !t.is_empty())
            && chrono::Utc::now().timestamp() - at < TOKEN_MAX_AGE_SECS;
        if fresh {
            return;
        }
        if let Ok(token) = login(API, &password).await {
            let _ = self.space_connect(&token).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Facts {
        Facts {
            os: "linux".into(),
            arch: "x86_64".into(),
            kvm: true,
            tun: true,
            docker: true,
            apt: true,
            root_or_sudo: true,
            missing_tools: vec![],
            free_gib: 120.0,
        }
    }

    #[test]
    fn a_ready_host_passes_every_check() {
        let c = evaluate(&ready());
        assert_eq!(c.len(), 8);
        assert!(blockers(&c).is_empty());
    }

    #[test]
    fn each_problem_is_named() {
        let mut f = ready();
        f.kvm = false;
        f.docker = false;
        f.missing_tools = vec!["git".into(), "openssl".into()];
        f.free_gib = 12.0;
        let c = evaluate(&f);
        let bad: Vec<&str> = blockers(&c).iter().map(|c| c.id).collect();
        assert_eq!(bad, vec!["kvm", "docker", "tools", "disk"]);
        assert!(c.iter().find(|x| x.id == "tools").unwrap().detail.contains("git, openssl"));
        let mac = Facts { os: "macos".into(), arch: "aarch64".into(), ..ready() };
        assert!(blockers(&evaluate(&mac)).iter().any(|c| c.id == "platform"));
    }

    #[test]
    fn passwords_are_long_and_different() {
        let (a, b) = (make_password(), make_password());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[tokio::test]
    async fn signs_in_against_a_local_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for _ in 0..2 {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let good = req.contains("\"password\":\"right\"");
                let (status, body) = if good {
                    ("200 OK", r#"{"token":"tok-123"}"#)
                } else {
                    ("401 Unauthorized", r#"{"error":"no"}"#)
                };
                let resp = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                s.write_all(resp.as_bytes()).await.unwrap();
            }
        });
        let api = format!("http://{addr}");
        assert_eq!(login(&api, "right").await.unwrap(), "tok-123");
        assert!(login(&api, "wrong").await.is_err());
    }
}
