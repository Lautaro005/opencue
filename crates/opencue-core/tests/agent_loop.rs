//! End-to-end: a mock OpenAI-compatible server asks for a `remember` tool call, then answers.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use opencue_core::models::{NewAgent, Role};
use opencue_core::paths::Paths;
use opencue_core::providers::{ProviderConfig, ProviderKind};
use opencue_core::{Core, CoreEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const REMEMBER_CALL: &[&str] = &[
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"remember","arguments":""}}]}}]}"#,
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"text\":\"Likes green tea\"}"}}]}}]}"#,
];
const NOTED_REPLY: &[&str] = &[
    r#"{"choices":[{"delta":{"content":"Noted, "}}]}"#,
    r#"{"choices":[{"delta":{"content":"you like green tea."}}]}"#,
    r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
];

async fn mock_llm() -> (String, Arc<AtomicUsize>) {
    mock_llm_with(REMEMBER_CALL, NOTED_REPLY).await
}

/// An OpenAI-compatible streaming server: the first request gets `first`, later ones `rest`.
async fn mock_llm_with(
    first: &'static [&'static str],
    rest: &'static [&'static str],
) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            let n = counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                let mut read = 0;
                // Read headers + body (Content-Length).
                loop {
                    let k = sock.read(&mut buf[read..]).await.unwrap();
                    read += k;
                    let text = String::from_utf8_lossy(&buf[..read]).to_string();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let len = text[..h]
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if read >= h + 4 + len {
                            break;
                        }
                    }
                    if k == 0 {
                        break;
                    }
                }
                let events = if n == 0 { first } else { rest };
                let mut body = String::new();
                for e in events {
                    body.push_str(&format!("data: {e}\n\n"));
                }
                body.push_str("data: [DONE]\n\n");
                let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                sock.write_all(resp.as_bytes()).await.unwrap();
                sock.shutdown().await.ok();
            });
        }
    });
    (format!("http://{addr}/v1"), hits)
}

#[tokio::test]
async fn agent_calls_tool_then_replies() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let (base, hits) = mock_llm().await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let agent = core
        .create_agent(NewAgent {
            name: "Tester".into(),
            provider_id: Some(provider.id),
            // Auto-memory (on by default) would make an extra model call after the reply.
            settings: Some(opencue_core::models::AgentSettings {
                auto_memory: false,
                auto_profile: false,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    let mut events = core.subscribe();
    core.send_message(&agent.id, "I like green tea")
        .await
        .unwrap();
    let mut deltas = String::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Ok(ev) = events.recv().await {
            if let CoreEvent::Delta { text, .. } = &ev {
                deltas.push_str(text);
            }
            if matches!(ev, CoreEvent::ThreadsChanged) && !core.is_busy(&agent.id) {
                break;
            }
        }
    })
    .await
    .expect("run finished");

    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(deltas, "Noted, you like green tea.");
    let memory = core.store.agent_memory(&agent.id).entries();
    assert_eq!(memory.len(), 1);
    assert_eq!(memory[0].text, "Likes green tea");

    let msgs = core.store.messages(&agent.id, 50).await.unwrap();
    let roles: Vec<Role> = msgs.iter().map(|m| m.role.clone()).collect();
    assert_eq!(
        roles,
        vec![Role::User, Role::Assistant, Role::Tool, Role::Assistant]
    );
    assert_eq!(msgs[2].status, "done");
    assert_eq!(msgs[3].content, "Noted, you like green tea.");
    assert_eq!(msgs[3].output_tokens, 5);
}

#[tokio::test]
async fn webhook_automation_runs_a_command_on_the_agents_computer() {
    use opencue_core::automations::{AutomationInput, Trigger};
    use opencue_core::computer::ComputerBackend;
    use opencue_core::models::{AgentSettings, ApprovalMode};

    const RUN_CALL: &[&str] = &[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"run_command","arguments":"{\"command\":\"echo hello-from-shell\"}"}}]}}]}"#,
    ];
    const DONE_REPLY: &[&str] = &[r#"{"choices":[{"delta":{"content":"The deploy hook ran."}}]}"#];

    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    core.store
        .set_setting("webhook_port", &port.to_string())
        .await
        .unwrap();
    core.start_background();

    let (base, hits) = mock_llm_with(RUN_CALL, DONE_REPLY).await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let settings = AgentSettings {
        approval_mode: ApprovalMode::Autonomous,
        computer: ComputerBackend::Host,
        auto_memory: false,
        auto_profile: false,
        ..Default::default()
    };
    let agent = core
        .create_agent(NewAgent {
            name: "Ops".into(),
            provider_id: Some(provider.id),
            settings: Some(settings),
            ..Default::default()
        })
        .await
        .unwrap();
    let token = opencue_core::automations::new_token();
    core.save_automation(AutomationInput {
        id: None,
        agent_id: agent.id.clone(),
        name: "Deploys".into(),
        trigger: Trigger::Webhook {
            token: token.clone(),
        },
        prompt: "Check the deploy.".into(),
        enabled: true,
    })
    .await
    .unwrap();

    // Give the webhook server a moment to bind, then call it like an external tool would.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let res = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/hooks/{token}"))
        .body("deploy ok")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 202);

    let msgs = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let msgs = core.store.messages(&agent.id, 50).await.unwrap();
            if msgs
                .iter()
                .any(|m| m.role == Role::Assistant && m.content == "The deploy hook ran.")
            {
                return msgs;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("automation ran");

    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(msgs[0].role, Role::Event);
    assert!(msgs[0].content.contains("Check the deploy."));
    assert!(msgs[0].content.contains("deploy ok"));
    let tool = msgs
        .iter()
        .find(|m| m.role == Role::Tool)
        .expect("tool result");
    assert_eq!(tool.status, "done");
    assert!(
        tool.content.contains("hello-from-shell"),
        "{}",
        tool.content
    );
    assert!(core
        .computer_log(&agent.id)
        .contains("$ echo hello-from-shell"));
    let a = core.automations(Some(&agent.id)).await.unwrap();
    assert_eq!(a[0].last_status.as_deref(), Some("ok"));
}

#[tokio::test]
async fn shared_files_reach_the_workspace_and_agents_share_files_back() {
    use base64::Engine;
    use opencue_core::computer::ComputerBackend;
    use opencue_core::models::{AgentSettings, NewAttachment};

    const SHARE_CALL: &[&str] = &[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"share_file","arguments":"{\"path\":\"files/notes.txt\",\"caption\":\"Here it is\"}"}}]}}]}"#,
    ];
    const DONE_REPLY: &[&str] = &[r#"{"choices":[{"delta":{"content":"Sent it back."}}]}"#];

    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let (base, _) = mock_llm_with(SHARE_CALL, DONE_REPLY).await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let agent = core
        .create_agent(NewAgent {
            name: "Clerk".into(),
            provider_id: Some(provider.id),
            settings: Some(AgentSettings {
                computer: ComputerBackend::Host,
                auto_memory: false,
                auto_profile: false,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    let sent = core
        .send_message_with_files(
            &agent.id,
            "",
            &[NewAttachment {
                name: "../notes.txt".into(),
                data: base64::engine::general_purpose::STANDARD.encode("hearing on the 9th"),
            }],
        )
        .await
        .unwrap();
    assert_eq!(sent.attachments.len(), 1);
    assert_eq!(sent.attachments[0].name, "notes.txt");
    let copy = core
        .paths()
        .agent_workspace(&agent.id)
        .join("files/notes.txt");
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        "hearing on the 9th"
    );

    let shared = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let files = core.thread_files(&agent.id).await.unwrap();
            if files.len() == 2 && !core.is_busy(&agent.id) {
                break files;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("agent shared the file back");
    assert_eq!(shared[0].agent_id.as_deref(), Some(agent.id.as_str()));
    let (att, bytes) = core
        .read_attachment(&shared[0].attachment.id)
        .await
        .unwrap();
    assert_eq!(att.name, "notes.txt");
    assert_eq!(bytes, b"hearing on the 9th");
}

/// A long tool result is saved in the agent's working folder (the one the user picked), and the
/// model only gets its start plus the path.
#[cfg(unix)]
#[tokio::test]
async fn long_tool_output_goes_to_a_file_in_the_chosen_working_folder() {
    use opencue_core::computer::ComputerBackend;
    use opencue_core::models::{AgentPatch, AgentSettings, ApprovalMode};

    const BIG_CALL: &[&str] = &[
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_big_1","function":{"name":"run_command","arguments":"{\"command\":\"head -c 20000 /dev/zero | tr '\\\\0' 'a'\"}"}}]}}]}"#,
    ];
    const DONE: &[&str] = &[r#"{"choices":[{"delta":{"content":"Done."}}]}"#];

    let dir = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let (base, _hits) = mock_llm_with(BIG_CALL, DONE).await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let settings = AgentSettings {
        approval_mode: ApprovalMode::Autonomous,
        computer: ComputerBackend::Host,
        auto_memory: false,
        auto_profile: false,
        working_dir: Some(work.path().display().to_string()),
        ..Default::default()
    };
    let agent = core
        .create_agent(NewAgent {
            name: "Ops".into(),
            provider_id: Some(provider.id),
            settings: Some(settings.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    // The folder goes through update_agent's validation too.
    core.update_agent(
        &agent.id,
        AgentPatch {
            settings: Some(settings),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let mut events = core.subscribe();
    core.send_message(&agent.id, "make a lot of output")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Ok(ev) = events.recv().await {
            if matches!(ev, CoreEvent::ThreadsChanged) && !core.is_busy(&agent.id) {
                break;
            }
        }
    })
    .await
    .expect("run finished");

    let msgs = core.store.messages(&agent.id, 50).await.unwrap();
    let tool = msgs
        .iter()
        .find(|m| m.role == Role::Tool)
        .expect("tool result");
    assert!(
        tool.content.contains(".opencue/outputs/"),
        "{}",
        &tool.content.chars().take(200).collect::<String>()
    );
    assert!(tool.content.chars().count() < 6_000);
    let saved = std::fs::read_dir(work.path().join(".opencue").join("outputs"))
        .expect("spill folder in the working folder")
        .count();
    assert_eq!(saved, 1);
    assert!(!core
        .paths()
        .agent_workspace(&agent.id)
        .join(".opencue")
        .exists());
}

const ASK_CALL: &[&str] = &[
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_ask","function":{"name":"ask_user","arguments":""}}]}}]}"#,
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"question\":\"Which color?\",\"options\":[\"blue\",\"red\"]}"}}]}}]}"#,
];
const TALK_CALL: &[&str] = &[
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_talk","function":{"name":"send_to_agent","arguments":""}}]}}]}"#,
    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"agent\":\"Helper\",\"message\":\"say hi\"}"}}]}}]}"#,
];

async fn agents_on(
    core: &Arc<Core>,
    first: &'static [&'static str],
    names: &[&str],
) -> Vec<opencue_core::models::Agent> {
    let (base, _) = mock_llm_with(first, NOTED_REPLY).await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let mut out = vec![];
    for name in names {
        out.push(
            core.create_agent(NewAgent {
                name: (*name).into(),
                provider_id: Some(provider.id.clone()),
                settings: Some(opencue_core::models::AgentSettings {
                    auto_memory: false,
                    auto_profile: false,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .unwrap(),
        );
    }
    out
}

async fn finish(
    core: &Arc<Core>,
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
    id: &str,
) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while let Ok(ev) = events.recv().await {
            if matches!(ev, CoreEvent::ThreadsChanged) && !core.is_busy(id) {
                break;
            }
        }
    })
    .await
    .expect("run finished");
}

#[tokio::test]
async fn ask_user_waits_for_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agent = agents_on(&core, ASK_CALL, &["Asker"]).await.remove(0);
    let mut events = core.subscribe();
    core.send_message(&agent.id, "pick for me").await.unwrap();
    // Answer the card when it shows up.
    let asked = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(CoreEvent::QuestionRequested { question }) = events.recv().await {
                return question;
            }
        }
    })
    .await
    .expect("a question was asked");
    assert_eq!(asked.body, "Which color?");
    assert_eq!(asked.options, ["blue", "red"]);
    assert_eq!(asked.thread_id, agent.id);
    assert!(core.answer_question(&asked.id, "blue").await);
    finish(&core, &mut events, &agent.id).await;
    let msgs = core.store.messages(&agent.id, 50).await.unwrap();
    let tool = msgs.iter().find(|m| m.role == Role::Tool).unwrap();
    assert_eq!(tool.content, "The user answered: blue");
}

#[tokio::test]
async fn an_agent_can_talk_to_another_one_without_a_group() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agents = agents_on(&core, TALK_CALL, &["Boss", "Helper"]).await;
    let (boss, helper) = (&agents[0], &agents[1]);
    let mut events = core.subscribe();
    core.send_message(&boss.id, "ask Helper to say hi")
        .await
        .unwrap();
    finish(&core, &mut events, &boss.id).await;

    let msgs = core.store.messages(&boss.id, 50).await.unwrap();
    let tool = msgs.iter().find(|m| m.role == Role::Tool).unwrap();
    assert!(
        tool.content.starts_with("[Helper]: Noted"),
        "the reply comes back as the result: {}",
        tool.content
    );
    // The exchange is kept as a conversation of its own, the same one from both sides.
    let dm = opencue_core::bridge::dm_thread_id(&boss.id, &helper.id);
    assert_eq!(dm, opencue_core::bridge::dm_thread_id(&helper.id, &boss.id));
    let talk = core.store.messages(&dm, 50).await.unwrap();
    assert_eq!(talk.len(), 2);
    assert_eq!(talk[0].agent_id.as_deref(), Some(boss.id.as_str()));
    assert_eq!(talk[0].content, "say hi");
    assert_eq!(talk[1].agent_id.as_deref(), Some(helper.id.as_str()));

    // Resetting the chat also forgets what the agent talked about with other agents.
    core.reset_thread(&boss.id).await.unwrap();
    assert!(core.store.messages(&boss.id, 50).await.unwrap().is_empty());
    assert!(core.store.messages(&dm, 50).await.unwrap().is_empty());
}

async fn quiet_agent(core: &Arc<Core>, provider: Option<String>) -> opencue_core::models::Agent {
    core.create_agent(NewAgent {
        name: "Maker".into(),
        provider_id: provider,
        settings: Some(opencue_core::models::AgentSettings {
            auto_memory: false,
            auto_profile: false,
            ..Default::default()
        }),
        ..Default::default()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn artifacts_are_files_with_versions() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agent = quiet_agent(&core, None).await;

    let made = core
        .call_external_tool(
            &agent.id,
            "create_artifact",
            json!({ "title": "Sales table", "type": "xlsx", "content": r#"{"sheets":[{"name":"Q1","rows":[["Item","Qty"],["Pen",3]]}]}"# }),
        )
        .await
        .unwrap();
    let id = made
        .split("artifact_id \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string();
    core.call_external_tool(
        &agent.id,
        "create_artifact",
        json!({ "artifact_id": id, "content": r#"{"rows":[["Item","Qty"],["Pen",4]]}"# }),
    )
    .await
    .unwrap();

    let artifact = core.get_artifact(&id).unwrap();
    assert_eq!(artifact.kind, "xlsx");
    assert_eq!(artifact.versions.len(), 2);
    assert_eq!(artifact.versions[1].attachment.name, "sales-table-v2.xlsx");
    // Every version is a real file that also shows up in the agent's Files.
    let files = core.thread_files(&agent.id).await.unwrap();
    assert_eq!(files.len(), 2);
    let (att, bytes) = core
        .read_attachment(&artifact.versions[0].attachment.id)
        .await
        .unwrap();
    assert_eq!(att.artifact.as_ref().unwrap().version, 1);
    assert!(bytes.starts_with(b"PK"));
    assert_eq!(core.list_artifacts(&agent.id).len(), 1);

    // Bad input is refused with a reason the agent can act on.
    let err = core
        .call_external_tool(
            &agent.id,
            "create_artifact",
            json!({ "title": "x", "type": "json", "content": "{nope" }),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not valid JSON"));
    let err = core
        .call_external_tool(
            &agent.id,
            "create_artifact",
            json!({ "artifact_id": id, "type": "pdf", "content": "# x" }),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("keeps its type"));
}

#[tokio::test]
async fn only_the_chosen_agent_edits_the_dashboard() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agent = quiet_agent(&core, None).await;
    let other = quiet_agent(&core, None).await;

    assert!(core.dashboard_is_default());
    assert!(core
        .call_external_tool(&agent.id, "update_dashboard", json!({ "html": "<p>x</p>" }))
        .await
        .is_err());

    core.store
        .set_setting("dashboard_agent", &agent.id)
        .await
        .unwrap();
    let page = core
        .call_external_tool(&agent.id, "read_dashboard", json!({}))
        .await
        .unwrap();
    assert!(page.contains("opencue:data"));
    core.call_external_tool(
        &agent.id,
        "update_dashboard",
        json!({ "html": "<h1>Mine</h1>" }),
    )
    .await
    .unwrap();
    assert_eq!(core.dashboard_html(), "<h1>Mine</h1>");
    assert!(!core.dashboard_is_default());
    assert!(core
        .call_external_tool(&other.id, "update_dashboard", json!({ "html": "<p>y</p>" }))
        .await
        .is_err());

    let data = core.dashboard_data().await.unwrap();
    assert_eq!(data["agents"]["total"], 2);
    assert_eq!(data["agents"]["running"], 0);
    assert!(data["tasks"].as_array().unwrap().is_empty());

    core.reset_dashboard().unwrap();
    assert!(core.dashboard_is_default());
}

#[tokio::test]
async fn regenerating_drops_the_old_answer_and_edits_the_question() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let (base, _hits) = mock_llm().await;
    let provider = core
        .save_provider(
            ProviderConfig {
                id: String::new(),
                kind: ProviderKind::OpenaiCompatible,
                name: "Mock".into(),
                base_url: Some(base),
                default_model: Some("mock-1".into()),
                enabled: true,
                has_key: false,
            },
            None,
        )
        .await
        .unwrap();
    let agent = quiet_agent(&core, Some(provider.id)).await;

    async fn wait_idle(core: &Arc<Core>, id: &str) {
        let mut events = core.subscribe();
        tokio::time::timeout(Duration::from_secs(10), async {
            while core.is_busy(id) {
                let _ = events.recv().await;
            }
        })
        .await
        .expect("run finished");
    }

    core.send_message(&agent.id, "first question")
        .await
        .unwrap();
    wait_idle(&core, &agent.id).await;
    assert_eq!(core.store.messages(&agent.id, 50).await.unwrap().len(), 4);

    core.regenerate_last(&agent.id, Some("edited question"))
        .await
        .unwrap();
    wait_idle(&core, &agent.id).await;
    let msgs = core.store.messages(&agent.id, 50).await.unwrap();
    let roles: Vec<Role> = msgs.iter().map(|m| m.role.clone()).collect();
    assert_eq!(roles, vec![Role::User, Role::Assistant]);
    assert_eq!(msgs[0].content, "edited question");
    assert_eq!(msgs[1].content, "Noted, you like green tea.");
}

#[tokio::test]
async fn the_council_answers_reviews_and_the_chair_decides() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agents = agents_on(&core, NOTED_REPLY, &["Ada", "Bo", "Cy"]).await;
    let ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
    let group = core
        .create_group_kind("Council", &ids, "council", serde_json::json!({}))
        .await
        .unwrap();
    let mut events = core.subscribe();
    // A plain message in a council group is a council question.
    core.send_message(&group.id, "Postgres or SQLite?")
        .await
        .unwrap();
    finish(&core, &mut events, &group.id).await;
    let msgs = core.store.messages(&group.id, 100).await.unwrap();
    let said = |who: &str| {
        msgs.iter()
            .filter(|m| m.role == Role::Assistant && m.agent_id.as_deref() == Some(who))
            .count()
    };
    // Opinion and review from everyone, and the chair (the first member) also writes the verdict.
    assert_eq!(said(&agents[0].id), 3);
    assert_eq!(said(&agents[1].id), 2);
    assert_eq!(said(&agents[2].id), 2);
    assert!(msgs
        .iter()
        .any(|m| m.role == Role::Event && m.content.contains("Council convened")));
    assert!(msgs
        .iter()
        .any(|m| m.role == Role::Event && m.content.starts_with("Verdict")));
    assert!(msgs
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .all(|m| m.status == "done"));

    // The command skips the review round.
    core.send_message(&group.id, "/ai-council --quick again?")
        .await
        .unwrap();
    finish(&core, &mut events, &group.id).await;
    let msgs = core.store.messages(&group.id, 100).await.unwrap();
    assert_eq!(
        msgs.iter().filter(|m| m.role == Role::Assistant).count(),
        7 + 4
    );
}

#[tokio::test]
async fn a_canvas_keeps_what_the_user_edits_and_the_agent_reads_it_back() {
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path()).unwrap()).await.unwrap();
    let agent = quiet_agent(&core, None).await;
    let made = core
        .call_external_tool(
            &agent.id,
            "create_artifact",
            json!({ "title": "Essay", "type": "markdown", "content": "# Draft", "canvas": true }),
        )
        .await
        .unwrap();
    assert!(made.contains("canvas"));
    let id = made
        .split("artifact_id \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string();
    let edited = core
        .save_canvas(&id, "# Draft\n\nmy own paragraph")
        .unwrap();
    assert_eq!(edited.versions.len(), 2);
    assert_eq!(edited.versions[1].by.as_deref(), Some("user"));
    // Saving the same text again is not a new version.
    assert_eq!(
        core.save_canvas(&id, "# Draft\n\nmy own paragraph")
            .unwrap()
            .versions
            .len(),
        2
    );
    let read = core
        .call_external_tool(&agent.id, "read_artifact", json!({ "artifact_id": id }))
        .await
        .unwrap();
    assert!(read.contains("edited by the user") && read.contains("my own paragraph"));
    // A plain artifact cannot be edited this way.
    let plain = core
        .call_external_tool(
            &agent.id,
            "create_artifact",
            json!({ "title": "P", "type": "text", "content": "x" }),
        )
        .await
        .unwrap();
    let pid = plain
        .split("artifact_id \"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(core.save_canvas(pid, "y").is_err());
}

#[tokio::test]
async fn coding_agents_get_the_repository_and_a_worktree_each() {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Paths::at(dir.path().join("data")).unwrap())
        .await
        .unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status();
        ok.map(|s| s.success()).unwrap_or(false)
    };
    if !git(&["init", "-q", "-b", "main"]) {
        return; // no git here
    }
    git(&["config", "user.email", "t@t"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);

    let agents = quiet_pair(&core).await;
    let ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
    let group = core
        .create_group_kind("Team", &ids, "coding", serde_json::json!({}))
        .await
        .unwrap();
    let status = core
        .coding_set_repo(&group.id, repo.to_str().unwrap(), true)
        .await
        .unwrap();
    assert!(status.is_repo && status.branch.as_deref() == Some("main"));
    let lead = core.store.get_agent(&agents[0].id).await.unwrap();
    let mate = core.store.get_agent(&agents[1].id).await.unwrap();
    // The lead works in the repository itself, teammates each in a worktree, all with a computer.
    assert_eq!(lead.settings.working_dir.as_deref(), repo.to_str());
    assert!(mate
        .settings
        .working_dir
        .as_deref()
        .unwrap()
        .contains("worktrees"));
    assert_eq!(
        mate.settings.computer,
        opencue_core::computer::ComputerBackend::Host
    );
    // A later member is prepared too.
    let third = core
        .create_agent(NewAgent {
            name: "Dee".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    core.add_group_member(&group.id, &third.id).await.unwrap();
    let third = core.store.get_agent(&third.id).await.unwrap();
    assert!(third
        .settings
        .working_dir
        .as_deref()
        .unwrap()
        .ends_with("dee"));
    // Not a repository: refused with a reason.
    assert!(core
        .coding_set_repo(&group.id, dir.path().to_str().unwrap(), true)
        .await
        .is_err());
    assert!(core.github_status(&group.id).await.unwrap().is_repo);
}

async fn quiet_pair(core: &Arc<Core>) -> Vec<opencue_core::models::Agent> {
    let mut out = vec![];
    for n in ["Lead", "Mate"] {
        out.push(
            core.create_agent(NewAgent {
                name: n.into(),
                ..Default::default()
            })
            .await
            .unwrap(),
        );
    }
    out
}
