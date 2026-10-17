//! Voice with ElevenLabs: Scribe speech-to-text in, the agent's normal reply, text-to-speech out.
//! The desktop app records the microphone and plays the audio; the core only talks to the API.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const BASE: &str = "https://api.elevenlabs.io/v1";
/// "Rachel", one of ElevenLabs' default voices.
pub const DEFAULT_VOICE: &str = "21m00Tcm4TlvDq8ikWAM";
pub const DEFAULT_TTS_MODEL: &str = "eleven_flash_v2_5";
pub const STT_MODEL: &str = "scribe_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub name: String,
}

pub struct ElevenLabs {
    http: reqwest::Client,
    key: String,
}

impl ElevenLabs {
    pub fn new(key: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(90))
                .build()
                .expect("http client"),
            key,
        }
    }

    async fn check(res: reqwest::Response) -> anyhow::Result<reqwest::Response> {
        if res.status().is_success() {
            return Ok(res);
        }
        let status = res.status();
        let body: Value = res.json().await.unwrap_or(Value::Null);
        let msg = body
            .pointer("/detail/message")
            .or_else(|| body.get("detail"))
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_else(|| "request failed".into());
        anyhow::bail!("ElevenLabs {status}: {msg}")
    }

    pub async fn voices(&self) -> anyhow::Result<Vec<Voice>> {
        let res = self
            .http
            .get(format!("{BASE}/voices"))
            .header("xi-api-key", &self.key)
            .send()
            .await?;
        let body: Value = Self::check(res).await?.json().await?;
        Ok(body
            .get("voices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| {
                Some(Voice {
                    id: v.get("voice_id")?.as_str()?.to_string(),
                    name: v.get("name")?.as_str()?.to_string(),
                })
            })
            .collect())
    }

    /// Audio bytes (webm/ogg/wav/mp3) → text.
    pub async fn transcribe(
        &self,
        audio: Vec<u8>,
        mime: &str,
        language: Option<&str>,
    ) -> anyhow::Result<String> {
        let ext = match mime {
            m if m.contains("ogg") => "ogg",
            m if m.contains("wav") => "wav",
            m if m.contains("mp4") || m.contains("m4a") => "m4a",
            m if m.contains("mpeg") || m.contains("mp3") => "mp3",
            _ => "webm",
        };
        let part = reqwest::multipart::Part::bytes(audio)
            .file_name(format!("speech.{ext}"))
            .mime_str(if mime.is_empty() { "audio/webm" } else { mime })?;
        let mut form = reqwest::multipart::Form::new()
            .text("model_id", STT_MODEL)
            .part("file", part);
        if let Some(l) = language.filter(|l| !l.is_empty()) {
            form = form.text("language_code", l.to_string());
        }
        let res = self
            .http
            .post(format!("{BASE}/speech-to-text"))
            .header("xi-api-key", &self.key)
            .multipart(form)
            .send()
            .await?;
        let body: Value = Self::check(res).await?.json().await?;
        Ok(body
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    /// Text → MP3 bytes.
    pub async fn speak(&self, text: &str, voice: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let voice = voice.filter(|v| !v.is_empty()).unwrap_or(DEFAULT_VOICE);
        let res = self
            .http
            .post(format!(
                "{BASE}/text-to-speech/{voice}?output_format=mp3_44100_128"
            ))
            .header("xi-api-key", &self.key)
            .json(&json!({ "text": text, "model_id": DEFAULT_TTS_MODEL }))
            .send()
            .await?;
        Ok(Self::check(res).await?.bytes().await?.to_vec())
    }
}

/// Markdown reads badly aloud: drop code blocks, links' URLs and formatting marks.
pub fn speakable(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let line = line
            .trim_start_matches(['#', '>', ' '])
            .trim_start_matches("- ");
        out.push_str(line);
        out.push('\n');
    }
    let link = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").expect("regex");
    let out = link.replace_all(&out, "$1");
    let out = out.replace(['*', '_', '`'], "");
    out.chars()
        .take(2500)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn strips_markdown_for_speech() {
        let s =
            super::speakable("## Hi\nSee [the docs](https://x.y) **now**.\n```\ncode\n```\n- done");
        assert_eq!(s, "Hi\nSee the docs now.\ndone");
    }
}
