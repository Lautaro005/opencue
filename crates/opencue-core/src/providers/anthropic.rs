//! Anthropic Messages API with streaming and tool use.

use std::collections::BTreeMap;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde_json::{json, Value};

use super::{
    error_for_status, http_client, ChatRequest, ChatResponse, ChatRole, DeltaSink, LlmProvider,
};
use crate::models::ToolCall;

const VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http: http_client(),
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", VERSION)
    }
}

/// Anthropic wants alternating user/assistant turns with tool results inside user turns.
fn to_anthropic_messages(req: &ChatRequest) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut push = |role: &str, blocks: Vec<Value>| {
        if let Some(last) = out.last_mut() {
            if last["role"] == role {
                last["content"]
                    .as_array_mut()
                    .expect("content array")
                    .extend(blocks);
                return;
            }
        }
        out.push(json!({ "role": role, "content": blocks }));
    };
    for m in &req.messages {
        match m.role {
            ChatRole::User => {
                let mut blocks: Vec<Value> = m
                    .images
                    .iter()
                    .map(|img| {
                        json!({
                            "type": "image",
                            "source": { "type": "base64", "media_type": img.mime, "data": img.data }
                        })
                    })
                    .collect();
                blocks.push(json!({ "type": "text", "text": non_empty(&m.content) }));
                push("user", blocks)
            }
            ChatRole::Assistant => {
                let mut blocks = Vec::new();
                if !m.content.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": m.content }));
                }
                for c in &m.tool_calls {
                    let input = if c.arguments.is_object() {
                        c.arguments.clone()
                    } else {
                        json!({})
                    };
                    blocks.push(
                        json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": input }),
                    );
                }
                if blocks.is_empty() {
                    blocks.push(json!({ "type": "text", "text": "…" }));
                }
                push("assistant", blocks);
            }
            ChatRole::Tool => push(
                "user",
                vec![json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": non_empty(&m.content),
                })],
            ),
        }
    }
    out
}

fn non_empty(s: &str) -> &str {
    if s.trim().is_empty() {
        "(empty)"
    } else {
        s
    }
}

#[derive(Default)]
struct Block {
    kind: String,
    id: String,
    name: String,
    json: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(
        &self,
        req: ChatRequest,
        on_delta: DeltaSink<'_>,
    ) -> anyhow::Result<ChatResponse> {
        let mut body = json!({
            "model": req.model,
            "max_tokens": 8192,
            "messages": to_anthropic_messages(&req),
            "stream": true,
        });
        if !req.system.is_empty() {
            body["system"] = json!(req.system);
        }
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if !req.tools.is_empty() {
            body["tools"] = req
                .tools
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.parameters }))
                .collect();
        }

        let resp = self
            .request(reqwest::Method::POST, "/v1/messages")
            .json(&body)
            .send()
            .await?;
        let resp = error_for_status(resp).await?;

        let mut out = ChatResponse::default();
        let mut blocks: BTreeMap<u64, Block> = BTreeMap::new();
        let mut stream = resp.bytes_stream().eventsource();
        while let Some(event) = stream.next().await {
            let event = event?;
            let Ok(data) = serde_json::from_str::<Value>(&event.data) else {
                continue;
            };
            match data["type"].as_str().unwrap_or(&event.event) {
                "message_start" => {
                    out.input_tokens = data
                        .pointer("/message/usage/input_tokens")
                        .and_then(Value::as_i64)
                        .unwrap_or(0);
                }
                "content_block_start" => {
                    let index = data["index"].as_u64().unwrap_or(0);
                    let cb = &data["content_block"];
                    blocks.insert(
                        index,
                        Block {
                            kind: cb["type"].as_str().unwrap_or_default().to_string(),
                            id: cb["id"].as_str().unwrap_or_default().to_string(),
                            name: cb["name"].as_str().unwrap_or_default().to_string(),
                            json: String::new(),
                        },
                    );
                }
                "content_block_delta" => {
                    let index = data["index"].as_u64().unwrap_or(0);
                    let delta = &data["delta"];
                    match delta["type"].as_str() {
                        Some("text_delta") => {
                            if let Some(t) = delta["text"].as_str() {
                                out.text.push_str(t);
                                on_delta(t);
                            }
                        }
                        Some("input_json_delta") => {
                            if let (Some(b), Some(p)) =
                                (blocks.get_mut(&index), delta["partial_json"].as_str())
                            {
                                b.json.push_str(p);
                            }
                        }
                        _ => {}
                    }
                }
                "message_delta" => {
                    if let Some(n) = data.pointer("/usage/output_tokens").and_then(Value::as_i64) {
                        out.output_tokens = n;
                    }
                }
                "error" => {
                    anyhow::bail!(
                        "Anthropic error: {}",
                        data.pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                    );
                }
                _ => {}
            }
        }

        out.tool_calls = blocks
            .into_values()
            .filter(|b| b.kind == "tool_use")
            .map(|b| ToolCall {
                id: b.id,
                name: b.name,
                arguments: if b.json.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&b.json).unwrap_or(json!({}))
                },
            })
            .collect();
        Ok(out)
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let resp = self
            .request(reqwest::Method::GET, "/v1/models?limit=100")
            .send()
            .await?;
        let resp = error_for_status(resp).await?;
        let v: Value = resp.json().await?;
        Ok(v["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ChatMessage;

    #[test]
    fn tool_results_merge_into_one_user_turn() {
        let req = ChatRequest {
            model: "m".into(),
            system: String::new(),
            messages: vec![
                ChatMessage::user("hi"),
                ChatMessage::assistant(
                    "",
                    vec![
                        ToolCall {
                            id: "a".into(),
                            name: "x".into(),
                            arguments: json!({}),
                        },
                        ToolCall {
                            id: "b".into(),
                            name: "y".into(),
                            arguments: json!({}),
                        },
                    ],
                ),
                ChatMessage::tool("a", "1"),
                ChatMessage::tool("b", "2"),
            ],
            tools: vec![],
            temperature: None,
            agent_id: None,
        };
        let msgs = to_anthropic_messages(&req);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[2]["content"].as_array().unwrap().len(), 2);
    }
}
