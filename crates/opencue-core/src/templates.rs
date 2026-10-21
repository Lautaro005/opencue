//! Importing Grok Bot templates from x.ai. A marketplace page (`x.ai/bot/marketplace/bots/<slug>`)
//! carries the whole template — instructions, memories, skills, routines and integrations — in the
//! data the page ships with; a shared link (`x.ai/bot/<id>`) only exposes the name, description
//! and look. Both are read from that data, nothing is run.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GrokSkill {
    pub name: String,
    pub description: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GrokRoutine {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GrokTemplate {
    pub id: String,
    pub name: String,
    pub creator: String,
    pub description: String,
    pub instructions: String,
    pub color: String,
    pub shape: String,
    pub memories: Vec<String>,
    pub skills: Vec<GrokSkill>,
    pub routines: Vec<GrokRoutine>,
    pub integrations: Vec<String>,
    /// `marketplace` pages have everything; `shared` links only a name and a description.
    pub source: String,
}

/// Which page to read: the address of a marketplace bot or a shared bot.
pub fn page_url(input: &str) -> anyhow::Result<String> {
    let raw = input.trim();
    anyhow::ensure!(!raw.is_empty(), "paste an x.ai/bot link");
    if !raw.contains('/') && !raw.contains('.') {
        return Ok(format!("https://x.ai/bot/{raw}"));
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| anyhow::anyhow!("that is not a link"))?;
    let host = url.host_str().unwrap_or_default();
    anyhow::ensure!(
        url.scheme() == "https" && (host == "x.ai" || host == "www.x.ai"),
        "only x.ai/bot links can be imported"
    );
    anyhow::ensure!(
        url.path().starts_with("/bot/"),
        "only x.ai/bot links can be imported"
    );
    Ok(format!("https://x.ai{}", url.path()))
}

/// The text of every `self.__next_f.push([1,"…"])` block in a page, decoded and joined.
fn flight_text(html: &str) -> String {
    let mut out = String::new();
    let marker = "self.__next_f.push([1,\"";
    let mut rest = html;
    while let Some(i) = rest.find(marker) {
        let body = &rest[i + marker.len()..];
        let bytes = body.as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            match bytes[j] {
                b'\\' => j += 2,
                b'"' => break,
                _ => j += 1,
            }
        }
        let literal = &body[..j.min(body.len())];
        if let Ok(s) = serde_json::from_str::<String>(&format!("\"{literal}\"")) {
            out.push_str(&s);
        }
        rest = &body[j.min(body.len())..];
    }
    out
}

/// The JSON object that starts at `start` (a `{`), found by matching braces outside strings.
fn object_at(s: &str, start: usize) -> Option<&str> {
    let bytes = s.as_bytes();
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for (k, &c) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=k]);
                }
            }
            _ => {}
        }
    }
    None
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Read a template out of a page's HTML.
pub fn parse_page(html: &str) -> anyhow::Result<GrokTemplate> {
    let flight = flight_text(html);
    // Marketplace pages: the whole template.
    if let Some(i) = flight.find("\"template\":{") {
        let start = i + "\"template\":".len();
        if let Some(obj) = object_at(&flight, start) {
            let v: Value = serde_json::from_str(obj)?;
            if v.get("name").is_some() {
                let list = |key: &str| {
                    v.get(key)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                };
                return Ok(GrokTemplate {
                    id: text(&v, "id"),
                    name: text(&v, "name"),
                    creator: text(&v, "creatorName"),
                    description: if text(&v, "description").is_empty() {
                        text(&v, "summary")
                    } else {
                        text(&v, "description")
                    },
                    instructions: text(&v, "instructions"),
                    color: text(&v, "color"),
                    shape: text(&v, "shape"),
                    memories: list("memories")
                        .iter()
                        .map(|m| text(m, "description"))
                        .filter(|m| !m.is_empty())
                        .collect(),
                    skills: list("skills")
                        .iter()
                        .map(|s| GrokSkill {
                            name: text(s, "name"),
                            description: text(s, "description"),
                            content: text(s, "content"),
                        })
                        .filter(|s| !s.name.is_empty())
                        .collect(),
                    routines: list("routines")
                        .iter()
                        .map(|r| GrokRoutine {
                            name: text(r, "name"),
                            summary: text(r, "summary"),
                        })
                        .filter(|r| !r.name.is_empty())
                        .collect(),
                    integrations: list("integrations")
                        .iter()
                        .map(|i| text(i, "name"))
                        .filter(|i| !i.is_empty())
                        .collect(),
                    source: "marketplace".into(),
                });
            }
        }
    }
    // Shared links: name, description and look.
    if let Some(i) = flight.find("\"botName\":") {
        let start = flight[..i].rfind('{').unwrap_or(0);
        if let Some(obj) = object_at(&flight, start) {
            let v: Value = serde_json::from_str(obj)?;
            return Ok(GrokTemplate {
                id: text(&v, "id"),
                name: text(&v, "botName"),
                creator: text(&v, "sharerName"),
                description: text(&v, "description"),
                color: text(&v, "color"),
                shape: text(&v, "shape"),
                source: "shared".into(),
                ..Default::default()
            });
        }
    }
    anyhow::bail!("that page has no bot template")
}

/// Fetch and read an x.ai bot link.
pub async fn import_grok_bot(input: &str) -> anyhow::Result<GrokTemplate> {
    let url = page_url(input)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (compatible; OpenCue)")
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()?;
    let resp = client.get(&url).send().await?;
    anyhow::ensure!(
        resp.status().is_success(),
        "x.ai answered {} for that link",
        resp.status()
    );
    let final_host = resp.url().host_str().unwrap_or_default().to_string();
    anyhow::ensure!(
        final_host == "x.ai" || final_host == "www.x.ai",
        "the link redirected away from x.ai"
    );
    let body = resp.bytes().await?;
    anyhow::ensure!(body.len() < 8 * 1024 * 1024, "the page is too big");
    parse_page(&String::from_utf8_lossy(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(payload: &str) -> String {
        // The page ships its data as JS string literals: quotes are escaped.
        let esc = serde_json::to_string(payload).unwrap();
        format!("<html><script>self.__next_f.push([1,{esc}])</script></html>")
    }

    #[test]
    fn a_marketplace_page_gives_everything() {
        let html = page(
            r#"3:["$","div",null,{"template":{"id":"qa-bot","name":"QA bot","creatorName":"Ulysses","description":"Runs the checklist.","instructions":"Be strict.","color":"magenta","shape":"teardrop","memories":[{"id":"m0","name":"memory 1","description":"Never soft-pass."}],"skills":[{"id":"s0","name":"fail-gate","description":"Use at ship time","content":"Step 1 {braces} \"quoted\""}],"routines":[{"id":"r0","name":"Post-merge","summary":"Re-run on merge."}],"integrations":[{"id":"i0","name":"Vercel","description":"Deploys"}]}}]"#,
        );
        let t = parse_page(&html).unwrap();
        assert_eq!(t.name, "QA bot");
        assert_eq!(t.source, "marketplace");
        assert_eq!(t.instructions, "Be strict.");
        assert_eq!(t.memories, ["Never soft-pass."]);
        assert_eq!(t.skills[0].name, "fail-gate");
        assert_eq!(t.skills[0].content, "Step 1 {braces} \"quoted\"");
        assert_eq!(t.routines[0].summary, "Re-run on merge.");
        assert_eq!(t.integrations, ["Vercel"]);
    }

    #[test]
    fn a_shared_link_gives_name_and_description() {
        let html = page(
            r##"4:{"state":{"data":{"id":"93g","ownerType":"USER","sharerName":"Lauren","botName":"dr eggbot","description":"Designs bots.","color":"#FF263C","shape":"teardrop"}}}"##,
        );
        let t = parse_page(&html).unwrap();
        assert_eq!(
            (t.name.as_str(), t.source.as_str()),
            ("dr eggbot", "shared")
        );
        assert_eq!(t.description, "Designs bots.");
        assert!(t.skills.is_empty());
        assert!(parse_page("<html>nothing</html>").is_err());
    }

    #[test]
    fn only_x_ai_bot_links_are_read() {
        assert_eq!(
            page_url("https://x.ai/bot/93g?x=1").unwrap(),
            "https://x.ai/bot/93g"
        );
        assert_eq!(
            page_url("x.ai/bot/marketplace/bots/qa-bot").unwrap(),
            "https://x.ai/bot/marketplace/bots/qa-bot"
        );
        assert_eq!(page_url("93gOz3op").unwrap(), "https://x.ai/bot/93gOz3op");
        assert!(page_url("https://evil.com/bot/1").is_err());
        assert!(page_url("https://x.ai/legal/privacy").is_err());
        assert!(page_url("http://x.ai/bot/1").is_err());
    }
}
