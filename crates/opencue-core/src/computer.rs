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

/// Options for an agent's Docker computer ("local VM"). Anything left empty uses the defaults from
/// Settings → Computers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct VmOptions {
    /// Run a desktop image and show it live; `None` follows Settings → Computers.
    #[serde(default)]
    pub desktop: Option<bool>,
    /// Memory limit in MB.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub cpus: Option<f32>,
    /// Folder that keeps the VM's home (desktop settings, browser profile, downloads…), so
    /// nothing is lost when the app closes, the VM stops or it is recreated. Empty uses a folder
    /// of its own inside OpenCue's data directory.
    #[serde(default)]
    pub data_dir: Option<String>,
    /// "docker" or "podman" for this agent; `None` follows Settings → Computers.
    #[serde(default)]
    pub runtime: Option<String>,
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
    /// One Docker computer (and desktop) for every agent instead of one each.
    #[serde(default)]
    pub shared: bool,
    /// Container engine: "docker" (default) or "podman".
    #[serde(default)]
    pub runtime: String,
    /// Minutes without use after which a Docker computer is stopped; 0 never stops it.
    #[serde(default)]
    pub idle_minutes: u32,
}

/// linuxserver.io's Webtop: maintained, multi-arch, serves an XFCE desktop to the browser.
pub const DEFAULT_DESKTOP_IMAGE: &str = "lscr.io/linuxserver/webtop:ubuntu-xfce";
pub const DEFAULT_DESKTOP_PORT: u16 = 3000;

/// The desktop Podman VMs use: Cua's XFCE image (what OpenMausBot runs), pinned by digest. The
/// linuxserver Webtop one needs privileges rootless Podman does not give and showed a black screen.
pub const CUA_IMAGE: &str =
    "docker.io/trycua/xfce-cua@sha256:274eb636f5cf3fc58f705916ee72b7a701270b3877369d08533a385c5325be9b";
/// Where Cua's image serves noVNC inside the container.
pub const CUA_PORT: u16 = 6901;
/// The viewer is published on 127.0.0.1 only; this just lets it connect without a prompt.
const CUA_PASSWORD: &str = "opencue";

/// The desktop image for an engine: Podman always gets Cua's, Docker the configured one.
pub fn desktop_image_for(rt: &str, configured: &str) -> String {
    if rt == "podman" {
        CUA_IMAGE.to_string()
    } else {
        match configured.trim() {
            "" => DEFAULT_DESKTOP_IMAGE.to_string(),
            s => s.to_string(),
        }
    }
}

pub fn desktop_port_for(rt: &str, configured: u16) -> u16 {
    if rt == "podman" {
        CUA_PORT
    } else if configured == 0 {
        DEFAULT_DESKTOP_PORT
    } else {
        configured
    }
}

/// The container engine behind "Local VM": Docker or Podman (rootless, no daemon).
pub fn runtime_name(s: &str) -> &'static str {
    if s.trim().eq_ignore_ascii_case("podman") {
        "podman"
    } else {
        "docker"
    }
}

/// Podman doesn't assume Docker Hub for short names, so spell them out.
pub fn qualify_image(rt: &str, image: &str) -> String {
    if rt != "podman" {
        return image.to_string();
    }
    let first = image.split('/').next().unwrap_or_default();
    let has_host =
        image.contains('/') && (first.contains('.') || first.contains(':') || first == "localhost");
    if has_host {
        image.to_string()
    } else if image.contains('/') {
        format!("docker.io/{image}")
    } else {
        format!("docker.io/library/{image}")
    }
}

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
    /// The Docker container is shared by every agent.
    pub shared: bool,
    pub workspace: PathBuf,
    pub vm: VmOptions,
    /// Where the VM's home is kept on this machine.
    pub vm_dir: PathBuf,
    pub config: &'a ComputerConfig,
    /// File that remembers this agent's SPACE sandbox id.
    pub space_state: PathBuf,
}

impl Computer<'_> {
    /// The engine this computer runs on: the agent's choice, else the global one.
    pub fn rt(&self) -> &'static str {
        runtime_name(self.vm.runtime.as_deref().unwrap_or(&self.config.runtime))
    }

    /// The desktop image and port for this computer's engine.
    pub fn desktop_image(&self) -> String {
        desktop_image_for(self.rt(), &self.config.desktop_image)
    }

    pub fn desktop_port(&self) -> u16 {
        desktop_port_for(self.rt(), self.config.desktop_port)
    }

    pub fn container_name(&self) -> String {
        if self.shared {
            return "opencue-shared".into();
        }
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
                let mut cmd = tokio::process::Command::new(self.rt());
                cmd.args(["exec", "-w", "/workspace"]);
                if self.desktop_enabled() {
                    // GUI programs the agent starts show up in the desktop view.
                    cmd.args(["-e", "DISPLAY=:1"]);
                    if self.rt() == "podman" {
                        // Cua's desktop runs as its `cua` user; its files belong to the host user.
                        cmd.args(["--user", "1000:1000", "-e", "HOME=/home/cua"]);
                    }
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

    /// Whether this agent's container runs a live desktop.
    pub fn desktop_enabled(&self) -> bool {
        self.vm.desktop.unwrap_or(self.config.docker_desktop)
    }

    /// Everything that is baked into the container when it is created; when it changes the
    /// container is recreated (the workspace and the VM folder are kept).
    fn fingerprint(&self) -> String {
        format!(
            "rt={};desktop={};image={};mem={};cpus={};vm={};ws={}",
            self.rt(),
            self.desktop_enabled(),
            if self.desktop_enabled() {
                self.desktop_image()
            } else {
                self.config.docker_image.trim().to_string()
            },
            self.vm.memory_mb.unwrap_or(0),
            self.vm.cpus.unwrap_or(0.0),
            self.vm_dir.display(),
            self.workspace.display()
        )
    }

    /// Create and start the Docker container if it isn't running.
    pub async fn start(&self) -> anyhow::Result<()> {
        self.ensure_container().await
    }

    async fn ensure_container(&self) -> anyhow::Result<()> {
        let name = self.container_name();
        let state = tokio::process::Command::new(self.rt())
            .args(["inspect", "-f", "{{.State.Running}}", &name])
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{} is not available ({e}). Install it and start it first.",
                    self.rt()
                )
            })?;
        let mut exists = state.status.success();
        let mut running = String::from_utf8_lossy(&state.stdout).trim() == "true";
        if exists {
            // Settings that are fixed at creation changed: start over with the same folders.
            let label = tokio::process::Command::new(self.rt())
                .args([
                    "inspect",
                    "-f",
                    "{{index .Config.Labels \"opencue.fp\"}}",
                    &name,
                ])
                .output()
                .await?;
            let have = String::from_utf8_lossy(&label.stdout).trim().to_string();
            if !have.is_empty() && have != "<no value>" && have != self.fingerprint() {
                self.reset().await?;
                exists = false;
                running = false;
            }
        }
        if running {
            return Ok(());
        }
        let out = if exists {
            tokio::process::Command::new(self.rt())
                .args(["start", &name])
                .output()
                .await?
        } else {
            let image = if self.config.docker_image.trim().is_empty() {
                "ubuntu:24.04"
            } else {
                self.config.docker_image.trim()
            };
            let image = qualify_image(self.rt(), image);
            std::fs::create_dir_all(&self.workspace)?;
            std::fs::create_dir_all(&self.vm_dir)?;
            // Podman relabels the folder for SELinux (a no-op elsewhere).
            let mount = if self.rt() == "podman" {
                format!("{}:/workspace:Z", self.workspace.display())
            } else {
                format!("{}:/workspace", self.workspace.display())
            };
            let mut cmd = tokio::process::Command::new(self.rt());
            cmd.args(["run", "-d", "--name", &name, "--label", "opencue=agent"]);
            cmd.args(["--label", &format!("opencue.fp={}", self.fingerprint())]);
            cmd.args(["-v", &mount]);
            if let Some(mb) = self.vm.memory_mb.filter(|m| *m > 0) {
                cmd.args(["--memory", &format!("{mb}m")]);
            }
            if let Some(cpus) = self.vm.cpus.filter(|c| *c > 0.0) {
                cmd.args(["--cpus", &cpus.to_string()]);
            }
            if self.desktop_enabled() && self.rt() == "podman" {
                // Cua's XFCE desktop, run the way OpenMausBot runs it on Podman: the supervisor
                // starts as root inside the user namespace and drops to `cua` (1000), which
                // keep-id maps to the host user so the workspace keeps its owner.
                cmd.args(["--label", "opencue.desktop=1"]);
                cmd.args(["--userns", "keep-id:uid=1000,gid=1000", "--user", "0:0"]);
                cmd.args(["--cap-drop", "ALL"]);
                for cap in ["SETUID", "SETGID", "SYS_CHROOT"] {
                    cmd.args(["--cap-add", cap]);
                }
                cmd.args(["--shm-size", "512m"]);
                cmd.args(["-e", &format!("VNC_PW={CUA_PASSWORD}")]);
                cmd.args(["-p", &format!("127.0.0.1::{CUA_PORT}")]);
                cmd.args(["--workdir", "/workspace"]);
                cmd.args([&qualify_image("podman", CUA_IMAGE)]);
            } else if self.desktop_enabled() {
                // The web desktop is published on a random port of 127.0.0.1 only; its home
                // (/config) lives in the VM folder so it survives restarts and recreation.
                let publish = format!("127.0.0.1::{}", self.desktop_port());
                cmd.args(["--label", "opencue.desktop=1", "-p", &publish]);
                cmd.args(["-v", &format!("{}:/config", self.vm_dir.display())]);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if let Ok(meta) = std::fs::metadata(&self.vm_dir) {
                        if meta.uid() != 0 {
                            cmd.args(["-e", &format!("PUID={}", meta.uid())]);
                            cmd.args(["-e", &format!("PGID={}", meta.gid())]);
                        }
                    }
                }
                cmd.args([
                    "--shm-size",
                    "1g",
                    &qualify_image(self.rt(), &self.desktop_image()),
                ]);
            } else {
                cmd.args(["-v", &format!("{}:/root", self.vm_dir.display())]);
                cmd.args(["-w", "/workspace", &image, "sleep", "infinity"]);
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
            self.desktop_enabled(),
            "turn on the live desktop in this agent's Computer settings first"
        );
        self.ensure_container().await?;
        let out = tokio::process::Command::new(self.rt())
            .args([
                "port",
                &self.container_name(),
                &format!("{}/tcp", self.desktop_port()),
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
        if self.rt() == "podman" {
            // Cua's noVNC page, already connected and scaled to the panel.
            return Ok(format!(
                "http://{}/vnc.html#autoconnect=true&resize=scale&password={CUA_PASSWORD}",
                addr.trim()
            ));
        }
        Ok(format!("http://{}/", addr.trim()))
    }

    /// Stop the container without removing it (the workspace and installed programs stay);
    /// the next command starts it again.
    pub async fn stop(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.backend == ComputerBackend::Docker,
            "only local VMs can be stopped from here"
        );
        let out = tokio::process::Command::new(self.rt())
            .args(["stop", &self.container_name()])
            .output()
            .await?;
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::ensure!(
            out.status.success() || err.contains("No such container"),
            "could not stop the VM: {}",
            err.trim()
        );
        Ok(())
    }

    /// Whether this computer's container is running right now.
    pub async fn is_running(&self) -> bool {
        if self.backend != ComputerBackend::Docker {
            return false;
        }
        running_containers(self.rt())
            .await
            .contains(&self.container_name())
    }

    /// Remove the Docker container (the workspace folder is kept); the next command recreates it.
    pub async fn reset(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.backend == ComputerBackend::Docker,
            "only Docker computers can be restarted from here"
        );
        let out = tokio::process::Command::new(self.rt())
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

/// Where the Docker setup stands, for the checklist in Settings → Computers.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DockerSetup {
    /// The `docker` command exists.
    pub installed: bool,
    /// The Docker engine answers (Docker Desktop / the daemon is started).
    pub running: bool,
    /// The desktop image is already downloaded.
    pub image_ready: bool,
    /// "none" | "stopped" | "running" for the computer named `container`.
    pub vm: String,
}

pub async fn docker_setup(rt: &str, image: &str, container: &str) -> DockerSetup {
    let image = qualify_image(rt, image);
    let mut s = DockerSetup {
        vm: "none".into(),
        ..Default::default()
    };
    s.installed = which::which(rt).is_ok();
    if !s.installed {
        return s;
    }
    let ok = |args: Vec<String>| async move {
        tokio::process::Command::new(rt)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .await
            .ok()
    };
    s.running = ok(vec!["info".into()])
        .await
        .is_some_and(|o| o.status.success());
    if !s.running {
        return s;
    }
    s.image_ready = ok(vec!["image".into(), "inspect".into(), image.clone()])
        .await
        .is_some_and(|o| o.status.success());
    if let Some(o) = ok(vec![
        "inspect".into(),
        "-f".into(),
        "{{.State.Running}}".into(),
        container.into(),
    ])
    .await
    .filter(|o| o.status.success())
    {
        s.vm = if String::from_utf8_lossy(&o.stdout).trim() == "true" {
            "running".into()
        } else {
            "stopped".into()
        };
    }
    s
}

/// Download an image, reporting progress through `sink`.
pub async fn pull_image(
    rt: &str,
    image: &str,
    sink: Sink<'_>,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let image = qualify_image(rt, image);
    anyhow::ensure!(
        which::which(rt).is_ok(),
        "{rt} is not installed. Install it first."
    );
    let mut cmd = tokio::process::Command::new(rt);
    cmd.args(["pull", &image]);
    let res = run_process(cmd, Duration::from_secs(60 * 60), sink, cancel).await?;
    anyhow::ensure!(res.exit_code == 0, "docker pull failed");
    Ok(())
}

/// Names of the running containers OpenCue created.
pub async fn running_containers(rt: &str) -> Vec<String> {
    let Ok(out) = tokio::process::Command::new(rt)
        .args([
            "ps",
            "--filter",
            "label=opencue=agent",
            "--format",
            "{{.Names}}",
        ])
        .output()
        .await
    else {
        return vec![];
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Run a one-off command on this computer (the Computers setup buttons), streaming its output.
pub async fn run_host_command(
    command: &str,
    sink: Sink<'_>,
    cancel: &CancellationToken,
) -> anyhow::Result<CommandResult> {
    run_process(
        shell_command(command),
        Duration::from_secs(900),
        sink,
        cancel,
    )
    .await
}

pub async fn stop_container(rt: &str, name: &str) {
    let _ = tokio::process::Command::new(rt)
        .args(["stop", name])
        .output()
        .await;
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
                shared: false,
                workspace: dir.join("ws"),
                vm: VmOptions::default(),
                vm_dir: dir.join("vm"),
                config: cfg,
                space_state: dir.join("space.json"),
            },
            cfg,
        )
    }

    #[test]
    fn podman_uses_cua_desktop_and_docker_the_configured_one() {
        assert_eq!(desktop_image_for("podman", "anything"), CUA_IMAGE);
        assert_eq!(desktop_port_for("podman", 3000), CUA_PORT);
        assert_eq!(desktop_image_for("docker", ""), DEFAULT_DESKTOP_IMAGE);
        assert_eq!(desktop_image_for("docker", "my/desktop"), "my/desktop");
        assert_eq!(desktop_port_for("docker", 0), DEFAULT_DESKTOP_PORT);
    }

    #[test]
    fn podman_gets_fully_qualified_image_names() {
        assert_eq!(qualify_image("docker", "ubuntu:24.04"), "ubuntu:24.04");
        assert_eq!(
            qualify_image("podman", "ubuntu:24.04"),
            "docker.io/library/ubuntu:24.04"
        );
        assert_eq!(qualify_image("podman", "foo/bar:1"), "docker.io/foo/bar:1");
        assert_eq!(
            qualify_image("podman", "lscr.io/linuxserver/webtop:ubuntu-xfce"),
            "lscr.io/linuxserver/webtop:ubuntu-xfce"
        );
        assert_eq!(
            qualify_image("podman", "localhost:5000/x"),
            "localhost:5000/x"
        );
        assert_eq!(runtime_name("Podman"), "podman");
        assert_eq!(runtime_name(""), "docker");
    }

    #[test]
    fn the_engine_follows_the_agent_then_the_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut c, _) = computer(tmp.path());
        assert_eq!(c.rt(), "docker");
        let base = c.fingerprint();
        c.vm.runtime = Some("podman".into());
        assert_eq!(c.rt(), "podman");
        assert_ne!(c.fingerprint(), base);
    }

    #[test]
    fn changing_the_vm_options_changes_the_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut c, _) = computer(tmp.path());
        let base = c.fingerprint();
        c.vm.memory_mb = Some(4096);
        assert_ne!(c.fingerprint(), base);
        let with_memory = c.fingerprint();
        c.vm.data_dir = Some("/somewhere".into());
        c.vm_dir = "/somewhere".into();
        assert_ne!(c.fingerprint(), with_memory);
        // The desktop switch falls back to the global setting.
        assert!(!c.desktop_enabled());
        c.vm.desktop = Some(true);
        assert!(c.desktop_enabled());
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
