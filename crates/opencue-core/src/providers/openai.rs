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

/// Mistral only accepts tool-call ids of exactly nine letters and digits, so ids made by other
/// providers (or earlier turns) are mapped to a stable nine-character one.
pub(crate) fn mistral_call_id(id: &str) -> String {
    if id.len() == 9 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return id.to_string();
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for b in id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    (0..9)
        .map(|_| {
            let c = ALPHABET[(h % 62) as usize] as char;
            h /= 62;
            c
        })
        .collect()
}

/// Text inside a delta's `content`: a string, or (Mistral's reasoning models) a list of chunks.
fn content_parts(content: &Value) -> (String, String) {
    match content {
        Value::String(s) => (String::new(), s.clone()),
        Value::Array(parts) => {
            let (mut thought, mut text) = (String::new(), String::new());
            for p in parts {
                match p["type"].as_str() {
                    Some("thinking") => {
                        for t in p["thinking"].as_array().into_iter().flatten() {
                            if let Some(s) = t["text"].as_str() {
                                thought.push_str(s);
                            }
                        }
                        if let Some(s) = p["thinking"].as_str() {
                            thought.push_str(s);
                        }
                    }
                    _ => {
                        if let Some(s) = p["text"].as_str() {
                            text.push_str(s);
                        }
                    }
                }
            }
            (thought, text)
        }
        _ => (String::new(), String::new()),
    }
}

pub(crate) fn to_openai_messages_for(req: &ChatRequest, mistral: bool) -> Vec<Value> {
    let call_id = |id: &str| {
        if mistral {
            mistral_call_id(id)
        } else {
            id.to_string()
        }
    };
    let mut out = Vec::with_capacity(req.messages.len() + 1);
    if !req.system.is_empty() {
        out.push(json!({ "role": "system", "content": req.system }));
    }
    for m in &req.messages {
        match m.role {
            ChatRole::User if !m.images.is_empty() => {
                let mut parts = vec![json!({ "type": "text", "text": m.content })];
                for img in &m.images {
                    parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": format!("data:{};base64,{}", img.mime, img.data) }
                    }));
                }
                out.push(json!({ "role": "user", "content": parts }));
            }
            ChatRole::User => out.push(json!({ "role": "user", "content": m.content })),
            ChatRole::Assistant => {
                let mut msg = json!({ "role": "assistant", "content": m.content });
                if !m.tool_calls.is_empty() {
                    msg["tool_calls"] = m
                        .tool_calls
                        .iter()
                        .map(|c| {
                            json!({
                                "id": call_id(&c.id),
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
                "tool_call_id": call_id(m.tool_call_id.as_deref().unwrap_or_default()),
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
        let mistral = self.kind == ProviderKind::MistralApi;
        let mut body = json!({
            "model": req.model,
            "messages": to_openai_messages_for(&req, mistral),
            "stream": true,
        });
        // Mistral rejects request fields it doesn't know and reports usage on its own.
        if !mistral {
            body["stream_options"] = json!({ "include_usage": true });
        }
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
        let mut think = super::ThinkStream::default();
        let mut watch = super::ToolWatch::default();
        let req_on_tool = req.on_tool.clone();
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
            // Reasoning models stream their thinking apart from the answer.
            if let Some(text) = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
            {
                let piece = think.reasoning(text);
                out.text.push_str(&piece);
                on_delta(&piece);
            }
            if let Some(content) = delta.get("content") {
                let (thought, text) = content_parts(content);
                if !thought.is_empty() {
                    let piece = think.reasoning(&thought);
                    out.text.push_str(&piece);
                    on_delta(&piece);
                }
                if !text.is_empty() {
                    let piece = think.answer(&text);
                    out.text.push_str(&piece);
                    on_delta(&piece);
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
                    let new_name = tc.pointer("/function/name").and_then(Value::as_str);
                    if let Some(name) = new_name {
                        entry.name.push_str(name);
                    }
                    watch.feed(
                        &req_on_tool,
                        index,
                        new_name.unwrap_or(""),
                        tc.pointer("/function/arguments").and_then(Value::as_str).unwrap_or(""),
                    );
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

        let closing = think.finish();
        if !closing.is_empty() {
            out.text.push_str(&closing);
            on_delta(&closing);
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
                            .map(|id| id.strip_prefix("models/").unwrap_or(id).to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Groq and Mistral list speech, embedding and moderation models next to the chat ones.
        if matches!(self.kind, ProviderKind::Groq | ProviderKind::MistralApi) {
            const NOT_CHAT: [&str; 8] = [
                "whisper",
                "tts",
                "orpheus",
                "embed",
                "moderation",
                "ocr",
                "guard",
                "transcribe",
            ];
            models.retain(|m| !NOT_CHAT.iter().any(|k| m.to_lowercase().contains(k)));
        }
        models.sort();
        models.dedup();
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mistral_ids_are_nine_alphanumerics_and_stable() {
        let a = mistral_call_id("call_0b3f-long-id");
        assert_eq!(a.len(), 9);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(a, mistral_call_id("call_0b3f-long-id"));
        assert_eq!(mistral_call_id("abc123XYZ"), "abc123XYZ");
    }

    #[test]
    fn content_chunks_split_reasoning_from_the_answer() {
        let v = json!([{"type":"thinking","thinking":[{"type":"text","text":"hm"}]},{"type":"text","text":"Hi"}]);
        assert_eq!(content_parts(&v), ("hm".to_string(), "Hi".to_string()));
        assert_eq!(content_parts(&json!("yo")).1, "yo");
    }
}
