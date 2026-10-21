//! `opencue-cli`: scripting companion to the OpenCue desktop app. Shares the same data directory.

use std::io::Write;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use opencue_core::models::NewAgent;
use opencue_core::providers::{ProviderConfig, ProviderKind};
use opencue_core::{Core, CoreEvent};

#[derive(Parser)]
#[command(name = "opencue-cli", version, about = "OpenCue from the terminal")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Manage agents
    #[command(subcommand)]
    Agents(AgentsCmd),
    /// Chat with an agent (interactive, or one-shot with --message)
    Chat {
        agent: String,
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Manage model providers
    #[command(subcommand)]
    Providers(ProvidersCmd),
    /// Manage MCP servers
    #[command(subcommand)]
    Mcp(McpCmd),
    /// Search the web with the built-in free search
    Search { query: Vec<String> },
    /// Print the data directory
    Where,
    /// Serve an agent's OpenCue tools over MCP stdio (used by `claude` / `codex` CLI agents)
    McpServe {
        #[arg(long)]
        agent: String,
    },
}

#[derive(Subcommand)]
enum AgentsCmd {
    List,
    Create {
        name: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        instructions: Option<String>,
    },
    Delete {
        agent: String,
    },
}

#[derive(Subcommand)]
enum ProvidersCmd {
    List,
    /// Add a provider, e.g. `providers add anthropic --key sk-...`
    Add {
        /// openai | anthropic | xai | openrouter | cheaperinference | ollama | ollama_cloud | lmstudio | openai_compatible | codex_cli | claude_cli | grok_cli | muse_cli | gemini | muse
        kind: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    Models {
        provider: String,
    },
}

#[derive(Subcommand)]
enum McpCmd {
    List,
    /// Import a Claude Desktop-style mcp.json file
    Import {
        file: std::path::PathBuf,
    },
    /// Add a remote MCP server by URL
    AddUrl {
        name: String,
        url: String,
    },
}

async fn resolve_agent(core: &Core, needle: &str) -> anyhow::Result<opencue_core::models::Agent> {
    if let Ok(a) = core.store.get_agent(needle).await {
        return Ok(a);
    }
    core.store
        .find_agent_by_name(needle)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no agent named {needle}"))
}

async fn resolve_provider(core: &Core, needle: &str) -> anyhow::Result<ProviderConfig> {
    core.providers()
        .await?
        .into_iter()
        .find(|p| {
            p.id == needle
                || p.name.eq_ignore_ascii_case(needle)
                || serde_json::to_value(p.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .as_deref()
                    == Some(needle)
        })
        .ok_or_else(|| anyhow::anyhow!("no provider matching {needle}"))
}

async fn chat_once(core: &Arc<Core>, agent_id: &str, text: &str) -> anyhow::Result<()> {
    let mut rx = core.subscribe();
    core.send_message(agent_id, text).await?;
    loop {
        match rx.recv().await {
            Ok(CoreEvent::Delta {
                thread_id, text, ..
            }) if thread_id == agent_id => {
                print!("{text}");
                std::io::stdout().flush()?;
            }
            Ok(CoreEvent::Message { message }) if message.thread_id == agent_id => {
                match message.role {
                    opencue_core::models::Role::Tool if message.status == "running" => {
                        eprintln!("\n  ⚙ {}", message.tool_name.unwrap_or_default());
                    }
                    opencue_core::models::Role::Assistant if message.status == "error" => {
                        eprintln!("\n✗ {}", message.content)
                    }
                    _ => {}
                }
            }
            Ok(CoreEvent::ApprovalRequested { approval }) if approval.thread_id == agent_id => {
                eprint!(
                    "\n? Allow {} ({})? [y/N] ",
                    approval.tool_call.name, approval.reason
                );
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                core.resolve_approval(&approval.id, line.trim().eq_ignore_ascii_case("y"));
            }
            Ok(CoreEvent::QuestionRequested { question }) if question.thread_id == agent_id => {
                eprint!("\n? {} {}", question.title, question.body);
                let permission = question.kind == "permission";
                eprint!("{} ", if permission { " [y/N]" } else { "" });
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                let line = line.trim();
                let answer = if permission {
                    if line.eq_ignore_ascii_case("y") {
                        opencue_core::bridge::APPROVED
                    } else {
                        opencue_core::bridge::REJECTED
                    }
                } else {
                    line
                };
                core.answer_question(&question.id, answer).await;
            }
            Ok(CoreEvent::ThreadsChanged) if !core.is_busy(agent_id) => break,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            _ => {}
        }
    }
    println!();
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    if let Cmd::McpServe { agent } = &cli.cmd {
        let core = Core::open_secondary(opencue_core::paths::Paths::resolve()?).await?;
        return opencue_core::mcp_server::serve_stdio(core, agent).await;
    }
    let core = Core::open_default().await?;

    match cli.cmd {
        Cmd::Where => println!("{}", core.paths().root().display()),
        Cmd::McpServe { .. } => unreachable!("handled above"),
        Cmd::Agents(AgentsCmd::List) => {
            for a in core.store.list_agents().await? {
                println!(
                    "{:<20} {}  {}",
                    a.name,
                    a.model.unwrap_or_else(|| "(provider default)".into()),
                    a.id
                );
            }
        }
        Cmd::Agents(AgentsCmd::Create {
            name,
            provider,
            model,
            instructions,
        }) => {
            let provider_id = match provider {
                Some(p) => Some(resolve_provider(&core, &p).await?.id),
                None => None,
            };
            let a = core
                .create_agent(NewAgent {
                    name,
                    provider_id,
                    model,
                    instructions,
                    ..Default::default()
                })
                .await?;
            println!("Created {} ({})", a.name, a.id);
        }
        Cmd::Agents(AgentsCmd::Delete { agent }) => {
            let a = resolve_agent(&core, &agent).await?;
            core.delete_agent(&a.id).await?;
            println!("Deleted {}", a.name);
        }
        Cmd::Chat { agent, message } => {
            core.start_background();
            let a = resolve_agent(&core, &agent).await?;
            if let Some(m) = message {
                chat_once(&core, &a.id, &m).await?;
            } else {
                println!("Chatting with {} — Ctrl+D to exit", a.name);
                loop {
                    print!("› ");
                    std::io::stdout().flush()?;
                    let mut line = String::new();
                    if std::io::stdin().read_line(&mut line)? == 0 {
                        break;
                    }
                    if !line.trim().is_empty() {
                        chat_once(&core, &a.id, line.trim()).await?;
                    }
                }
            }
        }
        Cmd::Providers(ProvidersCmd::List) => {
            for p in core.providers().await? {
                println!(
                    "{:<28} {:<18} key:{} model:{}  {}",
                    p.name,
                    serde_json::to_value(p.kind)?.as_str().unwrap_or(""),
                    if p.has_key { "yes" } else { "no " },
                    p.default_model.unwrap_or_default(),
                    p.id
                );
            }
        }
        Cmd::Providers(ProvidersCmd::Add {
            kind,
            name,
            key,
            base_url,
            model,
        }) => {
            let kind: ProviderKind = serde_json::from_value(serde_json::Value::String(kind))?;
            let info = opencue_core::providers::info(kind);
            let p = core
                .save_provider(
                    ProviderConfig {
                        id: String::new(),
                        kind,
                        name: name.unwrap_or_else(|| info.label.to_string()),
                        base_url,
                        default_model: model.or(info.default_model.map(str::to_string)),
                        enabled: true,
                        has_key: false,
                    },
                    key,
                )
                .await?;
            println!("Added {} ({})", p.name, p.id);
        }
        Cmd::Providers(ProvidersCmd::Models { provider }) => {
            let p = resolve_provider(&core, &provider).await?;
            for m in core.list_models(&p.id).await? {
                println!("{m}");
            }
        }
        Cmd::Mcp(McpCmd::List) => {
            for s in core.store.list_mcp_servers().await? {
                println!(
                    "{:<20} {:?} {}",
                    s.name,
                    s.transport,
                    s.url.or(s.command).unwrap_or_default()
                );
            }
        }
        Cmd::Mcp(McpCmd::Import { file }) => {
            let saved = core
                .import_mcp_json(&std::fs::read_to_string(file)?)
                .await?;
            println!("Imported {} server(s)", saved.len());
        }
        Cmd::Mcp(McpCmd::AddUrl { name, url }) => {
            core.save_mcp_server(opencue_core::models::McpServerConfig {
                id: String::new(),
                name,
                transport: opencue_core::models::McpTransport::Http,
                command: None,
                args: vec![],
                env: Default::default(),
                url: Some(url),
                headers: Default::default(),
                enabled: true,
            })
            .await?;
            println!("Added");
        }
        Cmd::Search { query } => {
            for r in core.web_search(&query.join(" "), 8).await? {
                println!("• {}\n  {}\n  {}\n", r.title, r.url, r.snippet);
            }
        }
    }
    Ok(())
}
