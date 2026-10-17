//! OpenAI-style `/chat/completions` (OpenAI, xAI, OpenRouter, Ollama, LM Studio, …).

use std::collections::BTreeMap;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde_json::{json, Value};

use super::{
    error_for_status, http_client, ChatRequest, ChatResponse, ChatRole, DeltaSink, LlmProvider,
    ProviderKind,
};
use crate::models::ToolCall;

pub struct OpenAiProvider {
    base_url: String,
    api_key: Option<String>,
    kind: ProviderKind,
    http: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(base_url: String, api_key: Option<String>, kind: ProviderKind) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            kind,
            http: http_client(),
        }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .http
            .request(method, format!("{}{}", self.base_url, path));
        if let Some(key) = self.api_key.as_deref().filter(|k| !k.is_empty()) {
            req = req.bearer_auth(key);
        }
        if self.kind == ProviderKind::Openrouter {
            req = req
                .header("HTTP-Referer", "https://github.com/Lautaro005/opencue")
                .header("X-Title", "OpenCue");
        }
        req
    }
}

pub(crate) fn to_openai_messages(req: &ChatRequest) -> Vec<Value> {
    let mut out = Vec::with_capacity(req.messages.len() + 1);
    if !req.system.is_empty() {
        out.push(json!({ "role": "system", "content": req.system }));
    }
    for m in &req.messages {
        match m.role {
            ChatRole::User => out.push(json!({ "role": "user", "content": m.content })),
            ChatRole::Assistant => {
                let mut msg = json!({ "role": "assistant", "content": m.content });
                if !m.tool_calls.is_empty() {
                    msg["tool_calls"] = m
                        .tool_calls
                        .iter()
                        .map(|c| {
                            json!({
                                "id": c.id,
                                "type": "function",
                                "function": { "name": c.name, "arguments": c.arguments.to_string() }
                            })
                        })
                        .collect();
                }
                out.push(msg);
            }
            ChatRole::Tool => out.push(json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                "content": m.content,
            })),
        }
    }
    out
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(
        &self,
        req: ChatRequest,
        on_delta: DeltaSink<'_>,
    ) -> anyhow::Result<ChatResponse> {
        let mut body = json!({
            "model": req.model,
            "messages": to_openai_messages(&req),
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if !req.tools.is_empty() {
            body["tools"] = req
                .tools
                .iter()
                .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.parameters } }))
                .collect();
        }

        let resp = self
            .request(reqwest::Method::POST, "/chat/completions")
            .json(&body)
            .send()
            .await?;
        let resp = error_for_status(resp).await?;

        let mut out = ChatResponse::default();
        let mut calls: BTreeMap<u64, PartialCall> = BTreeMap::new();
        let mut stream = resp.bytes_stream().eventsource();
        while let Some(event) = stream.next().await {
            let event = event?;
            if event.data.trim() == "[DONE]" {
                break;
            }
            let Ok(chunk) = serde_json::from_str::<Value>(&event.data) else {
                continue;
            };
            if let Some(err) = chunk.get("error") {
                anyhow::bail!(
                    "provider error: {}",
                    err.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or(&err.to_string())
                );
            }
            if let Some(usage) = chunk.get("usage").filter(|u| !u.is_null()) {
                out.input_tokens = usage["prompt_tokens"].as_i64().unwrap_or(out.input_tokens);
                out.output_tokens = usage["completion_tokens"]
                    .as_i64()
                    .unwrap_or(out.output_tokens);
            }
            let Some(delta) = chunk.pointer("/choices/0/delta") else {
                continue;
            };
            if let Some(text) = delta.get("content").and_then(Value::as_str) {
                if !text.is_empty() {
                    out.text.push_str(text);
                    on_delta(text);
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for (pos, tc) in tool_calls.iter().enumerate() {
                    let index = tc
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or(pos as u64);
                    let entry = calls.entry(index).or_default();
                    if let Some(id) = tc.get("id").and_then(Value::as_str) {
                        entry.id = id.to_string();
                    }
                    if let Some(name) = tc.pointer("/function/name").and_then(Value::as_str) {
                        entry.name.push_str(name);
                    }
                    if let Some(args) = tc.pointer("/function/arguments") {
                        match args {
                            Value::String(s) => entry.arguments.push_str(s),
                            // Some servers (Ollama) send the arguments as an object in one go.
                            other => entry.arguments = other.to_string(),
                        }
                    }
                }
            }
        }

        out.tool_calls = calls
            .into_values()
            .filter(|c| !c.name.is_empty())
            .map(|c| ToolCall {
                id: if c.id.is_empty() {
                    format!("call_{}", crate::db::new_id())
                } else {
                    c.id
                },
                name: c.name,
                arguments: if c.arguments.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&c.arguments)
                        .unwrap_or_else(|_| json!({ "_raw": c.arguments }))
                },
            })
            .collect();
        Ok(out)
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let resp = self.request(reqwest::Method::GET, "/models").send().await?;
        let resp = error_for_status(resp).await?;
        let v: Value = resp.json().await?;
        let mut models: Vec<String> = v["data"]
            .as_array()
            .or_else(|| v["models"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("id")
                            .or_else(|| m.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        models.sort();
        Ok(models)
    }
}
