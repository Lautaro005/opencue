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
