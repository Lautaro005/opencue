//! An agent's computer: where `run_command` and the file tools execute.
//!
//! - `host`: the user's own shell, inside the agent's workspace folder (asks before each command)
//! - `docker`: a long-lived container per agent with the workspace mounted at /workspace
//! - `wsl`: Windows Subsystem for Linux (Windows only; treated like the host)
//! - `space`: a Firecracker microVM on a SPACE server (https://github.com/perplexityai/SPACE)

use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComputerBackend {
    #[default]
    None,
    Host,
    Docker,
    Wsl,
    Space,
    /// Virtual machines from VMPal, driven through their MCP servers (no shell of its own).
    Vmpal,
}

impl ComputerBackend {
    /// Whether commands touch the user's real machine, so they need approval in "ask" mode.
    pub fn touches_host(&self) -> bool {
        matches!(self, ComputerBackend::Host | ComputerBackend::Wsl)
    }
}

/// Global computer settings (Settings → Computers).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComputerConfig {
    pub docker_image: String,
    /// Run Docker computers from a desktop image and show it in the Computer panel.
    #[serde(default)]
    pub docker_desktop: bool,
    /// Image with a web desktop (noVNC, KasmVNC or Selkies); empty uses [`DEFAULT_DESKTOP_IMAGE`].
    #[serde(default)]
    pub desktop_image: String,
    /// Port of the web desktop inside the container; 0 uses [`DEFAULT_DESKTOP_PORT`].
    #[serde(default)]
    pub desktop_port: u16,
    pub space_url: Option<String>,
    pub space_template: String,
    pub space_token: Option<String>,
}

/// linuxserver.io's Webtop: maintained, multi-arch, serves an XFCE desktop to the browser.
pub const DEFAULT_DESKTOP_IMAGE: &str = "lscr.io/linuxserver/webtop:ubuntu-xfce";
pub const DEFAULT_DESKTOP_PORT: u16 = 3000;

impl ComputerConfig {
    pub fn desktop_image(&self) -> &str {
        match self.desktop_image.trim() {
            "" => DEFAULT_DESKTOP_IMAGE,
            s => s,
        }
    }

    pub fn desktop_port(&self) -> u16 {
        if self.desktop_port == 0 {
            DEFAULT_DESKTOP_PORT
        } else {
            self.desktop_port
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: i32,
    pub output: String,
    pub timed_out: bool,
}

impl CommandResult {
    pub fn render(&self) -> String {
        let mut s = self.output.trim_end().to_string();
        if s.is_empty() {
            s = "(no output)".into();
        }
        if self.timed_out {
            s.push_str("\n[timed out]");
        }
        format!("{s}\n[exit code {}]", self.exit_code)
    }
}

/// Where command output goes while it runs: (is_stderr, text).
pub type Sink<'a> = &'a (dyn Fn(bool, &str) + Send + Sync);

const MAX_CAPTURE: usize = 200_000;

pub struct Computer<'a> {
    pub backend: ComputerBackend,
    pub agent_id: &'a str,
    pub workspace: PathBuf,
    pub config: &'a ComputerConfig,
    /// File that remembers this agent's SPACE sandbox id.
    pub space_state: PathBuf,
}

impl Computer<'_> {
    pub fn container_name(&self) -> String {
        format!(
            "opencue-{}",
            self.agent_id.chars().take(12).collect::<String>()
        )
    }

    pub async fn run(
        &self,
        command: &str,
        timeout: Duration,
        sink: Sink<'_>,
        cancel: &CancellationToken,
    ) -> anyhow::Result<CommandResult> {
        std::fs::create_dir_all(&self.workspace)?;
        match self.backend {
            ComputerBackend::None | ComputerBackend::Vmpal => {
                anyhow::bail!("this agent has no computer; pick one in its profile")
            }
            ComputerBackend::Host => {
                let mut cmd = shell_command(command);
                cmd.current_dir(&self.workspace);
                run_process(cmd, timeout, sink, cancel).await
            }
            ComputerBackend::Wsl => {
                let mut cmd = tokio::process::Command::new("wsl.exe");
                cmd.args(["-e", "sh", "-lc", command])
                    .current_dir(&self.workspace);
                run_process(cmd, timeout, sink, cancel).await
            }
            ComputerBackend::Docker => {
                self.ensure_container().await?;
                let mut cmd = tokio::process::Command::new("docker");
                cmd.args(["exec", "-w", "/workspace"]);
                if self.config.docker_desktop {
                    // GUI programs the agent starts show up in the desktop view.
                    cmd.args(["-e", "DISPLAY=:1"]);
                }
                cmd.args([&self.container_name(), "sh", "-lc", command]);
                run_process(cmd, timeout, sink, cancel).await
            }
            ComputerBackend::Space => {
                let space = self.space()?;
                let id = space.ensure_sandbox(&self.space_state).await?;
                space
                    .exec(
                        &id,
                        vec!["sh".into(), "-lc".into(), command.into()],
                        timeout,
                        sink,
                        cancel,
                    )
                    .await
            }
        }
    }

    async fn ensure_container(&self) -> anyhow::Result<()> {
        let name = self.container_name();
        let state = tokio::process::Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", &name])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Docker is not available ({e}). Install Docker Desktop or Docker Engine."
                )
            })?;
        let running = String::from_utf8_lossy(&state.stdout).trim().to_string();
        if running == "true" {
            return Ok(());
        }
        let out = if state.status.success() {
            tokio::process::Command::new("docker")
                .args(["start", &name])
                .output()
                .await?
        } else {
            let image = if self.config.docker_image.trim().is_empty() {
                "ubuntu:24.04"
            } else {
                self.config.docker_image.trim()
            };
            let mount = format!("{}:/workspace", self.workspace.display());
            let mut cmd = tokio::process::Command::new("docker");
            cmd.args(["run", "-d", "--name", &name, "--label", "opencue=agent"]);
            cmd.args(["-v", &mount]);
            if self.config.docker_desktop {
                // The web desktop is published on a random port of 127.0.0.1 only.
                let publish = format!("127.0.0.1::{}", self.config.desktop_port());
                cmd.args(["--label", "opencue.desktop=1", "-p", &publish]);
                cmd.args(["--shm-size", "1g", self.config.desktop_image()]);
            } else {
                cmd.args(["-w", "/workspace", image, "sleep", "infinity"]);
            }
            cmd.output().await?
        };
        anyhow::ensure!(
            out.status.success(),
            "could not start the container: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(())
    }

    /// Address of the Docker computer's web desktop on this machine.
    pub async fn desktop_url(&self) -> anyhow::Result<String> {
        anyhow::ensure!(
            self.backend == ComputerBackend::Docker,
            "the desktop view needs a Docker computer"
        );
        anyhow::ensure!(
            self.config.docker_desktop,
            "turn on the desktop in Settings → Computers first"
        );
        self.ensure_container().await?;
        let out = tokio::process::Command::new("docker")
            .args([
                "port",
                &self.container_name(),
                &format!("{}/tcp", self.config.desktop_port()),
            ])
            .output()
            .await?;
        let mapping = String::from_utf8_lossy(&out.stdout);
        let addr = mapping
            .lines()
            .find(|l| l.starts_with("127.0.0.1:"))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "this computer was created without a desktop; restart it to add one"
                )
            })?;
        Ok(format!("http://{}/", addr.trim()))
    }

    /// Remove the Docker container (the workspace folder is kept); the next command recreates it.
    pub async fn reset(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.backend == ComputerBackend::Docker,
            "only Docker computers can be restarted from here"
        );
        let out = tokio::process::Command::new("docker")
            .args(["rm", "-f", &self.container_name()])
            .output()
            .await?;
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::ensure!(
            out.status.success() || err.contains("No such container"),
            "could not remove the container: {}",
            err.trim()
        );
        Ok(())
    }

    fn space(&self) -> anyhow::Result<SpaceClient> {
        let url = self
            .config
            .space_url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("SPACE is not configured. Add its URL in Settings → Computers.")
            })?;
        SpaceClient::new(
            url,
            self.config.space_token.clone(),
            &self.config.space_template,
        )
    }

    /// The local file behind a workspace path (host, Docker and WSL share the folder).
    pub fn resolve(&self, rel: &str) -> anyhow::Result<PathBuf> {
        self.local_path(rel)
    }

    /// Files live in the workspace folder for host/docker/wsl, and inside the VM for SPACE.
    fn local_path(&self, rel: &str) -> anyhow::Result<PathBuf> {
        let rel = rel
            .trim()
            .trim_start_matches("/workspace")
            .trim_start_matches('/');
        let p = Path::new(rel);
        anyhow::ensure!(
            p.components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
            "paths must stay inside the workspace"
        );
        Ok(self.workspace.join(p))
    }

    pub async fn list(
        &self,
        rel: &str,
        cancel: &CancellationToken,
    ) -> anyhow::Result<Vec<FileEntry>> {
        if self.backend == ComputerBackend::Space {
            let dir = if rel.trim().is_empty() {
                "."
            } else {
                rel.trim()
            };
            let script = "cd \"$1\" 2>/dev/null || exit 3; for f in * .[!.]*; do [ -e \"$f\" ] || continue; \
                          if [ -d \"$f\" ]; then printf 'd\\t0\\t%s\\n' \"$f\"; else printf 'f\\t%s\\t%s\\n' \"$(wc -c < \"$f\")\" \"$f\"; fi; done";
            let res = self
                .space_argv(&["sh", "-c", script, "_", dir], cancel)
                .await?;
            anyhow::ensure!(res.exit_code == 0, "no such folder: {dir}");
            let mut out: Vec<FileEntry> = res
                .output
                .lines()
                .filter_map(|l| {
                    let mut it = l.splitn(3, '\t');
                    let (kind, size, name) = (it.next()?, it.next()?, it.next()?);
                    Some(FileEntry {
                        name: name.to_string(),
                        path: if dir == "." {
                            name.to_string()
                        } else {
                            format!("{}/{name}", dir.trim_end_matches('/'))
                        },
                        is_dir: kind == "d",
                        size: size.trim().parse().unwrap_or(0),
                    })
                })
                .collect();
            sort_entries(&mut out);
            return Ok(out);
        }
        std::fs::create_dir_all(&self.workspace)?;
        let dir = self.local_path(rel)?;
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir)?.flatten() {
            let meta = e.metadata()?;
            let name = e.file_name().to_string_lossy().to_string();
            let path = e
                .path()
                .strip_prefix(&self.workspace)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| name.clone());
            out.push(FileEntry {
                name,
                path,
                is_dir: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
            });
        }
        sort_entries(&mut out);
        Ok(out)
    }

    pub async fn read(&self, rel: &str, cancel: &CancellationToken) -> anyhow::Result<String> {
        if self.backend == ComputerBackend::Space {
            let res = self
                .space_argv(
                    &["sh", "-c", "head -c 200000 -- \"$1\"", "_", rel.trim()],
                    cancel,
                )
                .await?;
            anyhow::ensure!(res.exit_code == 0, "{}", res.output.trim());
            return Ok(res.output);
        }
        let bytes = std::fs::read(self.local_path(rel)?)?;
        let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_CAPTURE)]).to_string();
        Ok(text)
    }

    pub async fn write(
        &self,
        rel: &str,
        content: &str,
        cancel: &CancellationToken,
    ) -> anyhow::Result<()> {
        if self.backend == ComputerBackend::Space {
            let b64 = base64::engine::general_purpose::STANDARD.encode(content);
            let script =
                "mkdir -p \"$(dirname -- \"$1\")\" && printf %s \"$2\" | base64 -d > \"$1\"";
            let res = self
                .space_argv(&["sh", "-c", script, "_", rel.trim(), &b64], cancel)
                .await?;
            anyhow::ensure!(res.exit_code == 0, "{}", res.output.trim());
            return Ok(());
        }
        let path = self.local_path(rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    async fn space_argv(
        &self,
        argv: &[&str],
        cancel: &CancellationToken,
    ) -> anyhow::Result<CommandResult> {
        let space = self.space()?;
        let id = space.ensure_sandbox(&self.space_state).await?;
        space
            .exec(
                &id,
                argv.iter().map(|s| s.to_string()).collect(),
                Duration::from_secs(60),
                &|_, _| {},
                cancel,
            )
            .await
    }
}

fn sort_entries(v: &mut [FileEntry]) {
    v.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

fn shell_command(command: &str) -> tokio::process::Command {
    if cfg!(windows) {
        let mut c = tokio::process::Command::new("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-lc", command]);
        c
    }
}

async fn run_process(
    mut cmd: tokio::process::Command,
    timeout: Duration,
    sink: Sink<'_>,
    cancel: &CancellationToken,
) -> anyhow::Result<CommandResult> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn()?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let mut output = String::new();
    let (mut obuf, mut ebuf) = ([0u8; 4096], [0u8; 4096]);
    let (mut out_open, mut err_open) = (true, true);
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut timed_out = false;
    while out_open || err_open {
        tokio::select! {
            n = stdout.read(&mut obuf), if out_open => match n {
                Ok(0) | Err(_) => out_open = false,
                Ok(n) => push(&mut output, false, &String::from_utf8_lossy(&obuf[..n]), sink),
            },
            n = stderr.read(&mut ebuf), if err_open => match n {
                Ok(0) | Err(_) => err_open = false,
                Ok(n) => push(&mut output, true, &String::from_utf8_lossy(&ebuf[..n]), sink),
            },
            _ = &mut deadline => { timed_out = true; break; }
            _ = cancel.cancelled() => { let _ = child.kill().await; anyhow::bail!("stopped"); }
        }
    }
    if timed_out {
        let _ = child.kill().await;
        return Ok(CommandResult {
            exit_code: -1,
            output,
            timed_out,
        });
    }
    let status = child.wait().await?;
    Ok(CommandResult {
        exit_code: status.code().unwrap_or(-1),
        output,
        timed_out,
    })
}

fn push(output: &mut String, is_err: bool, text: &str, sink: Sink<'_>) {
    sink(is_err, text);
    if output.len() < MAX_CAPTURE {
        output.push_str(text);
    }
}

/// Minimal client for the SPACE control plane (`/v1`) and its SSE data plane.
pub struct SpaceClient {
    http: reqwest::Client,
    base: url::Url,
    token: Option<String>,
    template: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SpaceState {
    sandbox_id: Option<String>,
}

impl SpaceClient {
    pub fn new(url: &str, token: Option<String>, template: &str) -> anyhow::Result<Self> {
        let mut s = url.trim().trim_end_matches('/').to_string();
        if !(s.ends_with("/v1")) {
            s.push_str("/v1");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(15))
                .build()?,
            base: url::Url::parse(&format!("{s}/"))?,
            token: token.filter(|t| !t.trim().is_empty()),
            template: if template.trim().is_empty() {
                "seed:latest".into()
            } else {
                template.trim().into()
            },
        })
    }

    fn req(&self, method: reqwest::Method, path: &str) -> anyhow::Result<reqwest::RequestBuilder> {
        let url = self.base.join(path.trim_start_matches('/'))?;
        let mut r = self
            .http
            .request(method, url)
            .timeout(Duration::from_secs(60));
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        Ok(r)
    }

    async fn send(r: reqwest::RequestBuilder) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        let res = r.send().await?;
        let status = res.status();
        let body: Value = res.json().await.unwrap_or(Value::Null);
        Ok((status, body))
    }

    fn fail(status: reqwest::StatusCode, body: &Value) -> anyhow::Error {
        let msg = body
            .pointer("/error/message")
            .or_else(|| body.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("request failed");
        anyhow::anyhow!("SPACE {status}: {msg}")
    }

    /// Check the server answers (used by Settings → Computers → Test).
    pub async fn ping(&self) -> anyhow::Result<usize> {
        let (status, body) = Self::send(self.req(reqwest::Method::GET, "sandboxes")?).await?;
        if !status.is_success() {
            return Err(Self::fail(status, &body));
        }
        Ok(body
            .get("data")
            .and_then(Value::as_array)
            .map_or(0, Vec::len))
    }

    async fn wait_operation(&self, op: &str) -> anyhow::Result<()> {
        for _ in 0..120 {
            let (status, body) =
                Self::send(self.req(reqwest::Method::GET, &format!("operations/{op}"))?).await?;
            if !status.is_success() {
                return Err(Self::fail(status, &body));
            }
            match body.get("state").and_then(Value::as_str).unwrap_or("") {
                "succeeded" => return Ok(()),
                "failed" | "canceled" | "expired" => anyhow::bail!(
                    "SPACE operation {op} ended as {}: {}",
                    body["state"],
                    body.get("terminal_error")
                        .map(|e| e.to_string())
                        .unwrap_or_default()
                ),
                _ => tokio::time::sleep(Duration::from_secs(1)).await,
            }
        }
        anyhow::bail!("SPACE operation {op} is taking too long")
    }

    /// Reuse the agent's sandbox (waking it if needed) or create one.
    pub async fn ensure_sandbox(&self, state_file: &Path) -> anyhow::Result<String> {
        let mut state: SpaceState = std::fs::read_to_string(state_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if let Some(id) = state.sandbox_id.clone() {
            let (status, body) =
                Self::send(self.req(reqwest::Method::GET, &format!("sandboxes/{id}"))?).await?;
            if status.is_success() {
                match body.get("state").and_then(Value::as_str).unwrap_or("") {
                    "running" => return Ok(id),
                    "suspended" | "paused" | "stopped" => {
                        let (s, b) = Self::send(
                            self.req(reqwest::Method::POST, &format!("sandboxes/{id}:wake"))?
                                .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
                                .json(&json!({})),
                        )
                        .await?;
                        if s.is_success() {
                            if let Some(op) = b.get("operation_id").and_then(Value::as_str) {
                                self.wait_operation(op).await?;
                            }
                            return Ok(id);
                        }
                    }
                    "creating" | "starting" | "resuming" | "restoring" => {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        return Ok(id);
                    }
                    _ => {}
                }
            }
        }
        let (status, body) = Self::send(
            self.req(reqwest::Method::POST, "sandboxes")?
                .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
                .json(&json!({ "template": self.template, "name": "opencue-agent", "metadata": { "app": "opencue" } })),
        )
        .await?;
        if !status.is_success() {
            return Err(Self::fail(status, &body));
        }
        let id = body
            .get("sandbox_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("SPACE did not return a sandbox id"))?
            .to_string();
        if let Some(op) = body.get("operation_id").and_then(Value::as_str) {
            self.wait_operation(op).await?;
        }
        state.sandbox_id = Some(id.clone());
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(state_file, serde_json::to_string(&state)?)?;
        Ok(id)
    }

    pub async fn exec(
        &self,
        sandbox: &str,
        argv: Vec<String>,
        timeout: Duration,
        sink: Sink<'_>,
        cancel: &CancellationToken,
    ) -> anyhow::Result<CommandResult> {
        let (status, body) = Self::send(
            self.req(
                reqwest::Method::POST,
                &format!("sandboxes/{sandbox}/commands"),
            )?
            .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .json(&json!({ "command": argv, "timeout": format!("{}s", timeout.as_secs().max(1)) })),
        )
        .await?;
        if !status.is_success() {
            return Err(Self::fail(status, &body));
        }
        let events = body
            .pointer("/attach/events_url")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("this SPACE server does not offer SSE output"))?;
        let token = body
            .pointer("/attach/token")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // Relative URLs resolve against the server's origin.
        let url = self.base.join(events)?;
        anyhow::ensure!(
            url.host_str() == self.base.host_str(),
            "SPACE returned an output stream on another host ({}); refusing to send it credentials",
            url.host_str().unwrap_or("?")
        );
        let res = self
            .http
            .get(url)
            .bearer_auth(token)
            .header("Accept", "text/event-stream")
            .send()
            .await?;
        anyhow::ensure!(
            res.status().is_success(),
            "SPACE output stream: {}",
            res.status()
        );
        let mut stream = eventsource_stream::Eventsource::eventsource(res.bytes_stream());
        let mut output = String::new();
        let deadline = tokio::time::sleep(timeout + Duration::from_secs(10));
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                ev = stream.next() => {
                    let Some(ev) = ev else { break };
                    let ev = ev?;
                    let Ok(frame) = serde_json::from_str::<Value>(&ev.data) else { continue };
                    match frame.get("type").and_then(Value::as_str).unwrap_or("") {
                        t @ ("stdout" | "stderr") => {
                            let data = frame.get("data_base64").and_then(Value::as_str).unwrap_or_default();
                            let bytes = base64::engine::general_purpose::STANDARD.decode(data).unwrap_or_default();
                            push(&mut output, t == "stderr", &String::from_utf8_lossy(&bytes), sink);
                        }
                        "process_exited" => {
                            let code = frame.get("exit_code").and_then(Value::as_i64).unwrap_or(-1) as i32;
                            return Ok(CommandResult { exit_code: code, output, timed_out: false });
                        }
                        "error" => anyhow::bail!("SPACE: {}", frame),
                        _ => {}
                    }
                }
                _ = &mut deadline => return Ok(CommandResult { exit_code: -1, output, timed_out: true }),
                _ = cancel.cancelled() => anyhow::bail!("stopped"),
            }
        }
        anyhow::bail!("SPACE closed the output stream before the command finished")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn computer(dir: &Path) -> (Computer<'static>, &'static ComputerConfig) {
        let cfg: &'static ComputerConfig = Box::leak(Box::default());
        (
            Computer {
                backend: ComputerBackend::Host,
                agent_id: "a1",
                workspace: dir.join("ws"),
                config: cfg,
                space_state: dir.join("space.json"),
            },
            cfg,
        )
    }

    #[test]
    fn workspace_paths_cannot_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let (c, _) = computer(tmp.path());
        assert!(c.local_path("../secrets.json").is_err());
        assert!(c.local_path("notes/today.md").is_ok());
        assert_eq!(
            c.local_path("/workspace/a.txt").unwrap(),
            tmp.path().join("ws/a.txt")
        );
    }

    #[tokio::test]
    async fn host_runs_commands_and_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (c, _) = computer(tmp.path());
        let cancel = CancellationToken::new();
        c.write("hello.txt", "hi there", &cancel).await.unwrap();
        assert_eq!(c.read("hello.txt", &cancel).await.unwrap(), "hi there");
        assert_eq!(c.list("", &cancel).await.unwrap()[0].name, "hello.txt");
        let cmd = if cfg!(windows) {
            "Get-Content hello.txt"
        } else {
            "cat hello.txt"
        };
        let res = c
            .run(cmd, Duration::from_secs(20), &|_, _| {}, &cancel)
            .await
            .unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(res.output.contains("hi there"));
    }
}
