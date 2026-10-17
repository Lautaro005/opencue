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

async fn mock_llm() -> (String, Arc<AtomicUsize>) {
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
                let events = if n == 0 {
                    vec![
                        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"remember","arguments":""}}]}}]}"#,
                        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"text\":\"Likes green tea\"}"}}]}}]}"#,
                    ]
                } else {
                    vec![
                        r#"{"choices":[{"delta":{"content":"Noted, "}}]}"#,
                        r#"{"choices":[{"delta":{"content":"you like green tea."}}]}"#,
                        r#"{"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
                    ]
                };
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
