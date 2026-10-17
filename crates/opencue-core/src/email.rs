//! Per-agent email over IMAP (read/search) and SMTP (send). Gmail through Composio is handled by
//! the runtime, which routes the same tools to Composio actions when the account kind says so.

use std::sync::Arc;

use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{self, pki_types::ServerName};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmailKind {
    #[default]
    Imap,
    ComposioGmail,
}

impl EmailKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EmailKind::Imap => "imap",
            EmailKind::ComposioGmail => "composio_gmail",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "composio_gmail" {
            EmailKind::ComposioGmail
        } else {
            EmailKind::Imap
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAccount {
    pub agent_id: String,
    #[serde(default)]
    pub kind: EmailKind,
    pub address: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub imap_host: Option<String>,
    #[serde(default = "imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default = "smtp_port")]
    pub smtp_port: u16,
    /// Login name when it differs from the address.
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub last_seen_uid: u32,
    /// Filled in by the core: whether a password is stored.
    #[serde(default)]
    pub has_password: bool,
}

fn imap_port() -> u16 {
    993
}
fn smtp_port() -> u16 {
    465
}

impl EmailAccount {
    fn login(&self) -> &str {
        self.username
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or(&self.address)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSummary {
    pub uid: u32,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub unread: bool,
    pub message_id: Option<String>,
    pub text: String,
}

impl EmailSummary {
    pub fn render(&self, full: bool) -> String {
        let text = if full {
            self.text.clone()
        } else {
            self.text.chars().take(400).collect::<String>()
        };
        format!(
            "#{} {}{}\nFrom: {}\nDate: {}\nSubject: {}\n\n{}",
            self.uid,
            if self.unread { "[unread] " } else { "" },
            self.message_id
                .as_deref()
                .map(|m| format!("<{m}>"))
                .unwrap_or_default(),
            self.from,
            self.date,
            self.subject,
            text.trim()
        )
    }
}

/// Well-known servers so most people only type their address and an app password.
pub fn guess_servers(address: &str) -> Option<(&'static str, &'static str)> {
    let domain = address.rsplit('@').next()?.to_lowercase();
    Some(match domain.as_str() {
        "gmail.com" | "googlemail.com" => ("imap.gmail.com", "smtp.gmail.com"),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => {
            ("outlook.office365.com", "smtp.office365.com")
        }
        "yahoo.com" | "ymail.com" => ("imap.mail.yahoo.com", "smtp.mail.yahoo.com"),
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com", "smtp.mail.me.com"),
        "fastmail.com" | "fastmail.fm" => ("imap.fastmail.com", "smtp.fastmail.com"),
        "proton.me" | "protonmail.com" => ("127.0.0.1", "127.0.0.1"), // Proton Bridge
        _ => return None,
    })
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("TLS versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    Arc::new(config)
}

type Session = async_imap::Session<tokio_rustls::client::TlsStream<TcpStream>>;

async fn connect(acc: &EmailAccount, password: &str) -> anyhow::Result<Session> {
    let host = acc
        .imap_host
        .as_deref()
        .filter(|h| !h.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("IMAP server is not set for {}", acc.address))?;
    let tcp = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        TcpStream::connect((host, acc.imap_port)),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out connecting to {host}"))??;
    let name = ServerName::try_from(host.to_string())?;
    let tls = tokio_rustls::TlsConnector::from(tls_config())
        .connect(name, tcp)
        .await?;
    let mut client = async_imap::Client::new(tls);
    client
        .read_response()
        .await?
        .ok_or_else(|| anyhow::anyhow!("IMAP server closed the connection"))?;
    client
        .login(acc.login(), password)
        .await
        .map_err(|(e, _)| anyhow::anyhow!("IMAP login failed: {e}"))
}

fn summarize(uid: u32, raw: &[u8], unread: bool) -> EmailSummary {
    let parsed = mail_parser::MessageParser::default().parse(raw);
    let Some(m) = parsed else {
        return EmailSummary {
            uid,
            from: String::new(),
            subject: "(unreadable message)".into(),
            date: String::new(),
            unread,
            message_id: None,
            text: String::new(),
        };
    };
    let from = m
        .from()
        .and_then(|a| a.first())
        .map(|a| match (a.name(), a.address()) {
            (Some(n), Some(e)) => format!("{n} <{e}>"),
            (None, Some(e)) => e.to_string(),
            (Some(n), None) => n.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();
    let mut text = m.body_text(0).map(|t| t.to_string()).unwrap_or_else(|| {
        m.body_preview(4000)
            .map(|t| t.to_string())
            .unwrap_or_default()
    });
    if text.chars().count() > 8000 {
        text = text.chars().take(8000).collect::<String>() + "\n…";
    }
    EmailSummary {
        uid,
        from,
        subject: m.subject().unwrap_or("(no subject)").to_string(),
        date: m.date().map(|d| d.to_rfc3339()).unwrap_or_default(),
        unread,
        message_id: m.message_id().map(str::to_string),
        text,
    }
}

async fn fetch_uids(
    session: &mut Session,
    mut uids: Vec<u32>,
    limit: usize,
) -> anyhow::Result<Vec<EmailSummary>> {
    uids.sort_unstable();
    let take: Vec<u32> = uids.into_iter().rev().take(limit).collect();
    if take.is_empty() {
        return Ok(vec![]);
    }
    let set = take
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let fetches: Vec<_> = session
        .uid_fetch(&set, "(UID FLAGS BODY.PEEK[])")
        .await?
        .try_collect()
        .await?;
    let mut out: Vec<EmailSummary> = fetches
        .iter()
        .filter_map(|f| {
            let uid = f.uid?;
            let unread = !f.flags().any(|fl| fl == async_imap::types::Flag::Seen);
            Some(summarize(uid, f.body()?, unread))
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.uid));
    Ok(out)
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Newest messages in INBOX, optionally only unread ones.
pub async fn recent(
    acc: &EmailAccount,
    password: &str,
    limit: usize,
    unread_only: bool,
) -> anyhow::Result<Vec<EmailSummary>> {
    let mut s = connect(acc, password).await?;
    s.examine("INBOX").await?;
    let uids: Vec<u32> = s
        .uid_search(if unread_only { "UNSEEN" } else { "ALL" })
        .await?
        .into_iter()
        .collect();
    let out = fetch_uids(&mut s, uids, limit).await;
    let _ = s.logout().await;
    out
}

/// Full-text search in INBOX (IMAP `TEXT`, or `FROM`/`SUBJECT` with a `from:`/`subject:` prefix).
pub async fn search(
    acc: &EmailAccount,
    password: &str,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<EmailSummary>> {
    let q = query.trim();
    let criteria = if let Some(v) = q.strip_prefix("from:") {
        format!("FROM {}", quote(v.trim()))
    } else if let Some(v) = q.strip_prefix("subject:") {
        format!("SUBJECT {}", quote(v.trim()))
    } else {
        format!("TEXT {}", quote(q))
    };
    let mut s = connect(acc, password).await?;
    s.examine("INBOX").await?;
    let uids: Vec<u32> = s.uid_search(&criteria).await?.into_iter().collect();
    let out = fetch_uids(&mut s, uids, limit).await;
    let _ = s.logout().await;
    out
}

/// Messages newer than `after_uid` (for automations). The first call only records the current
/// high-water mark so old mail doesn't fire anything.
pub async fn new_since(
    acc: &EmailAccount,
    password: &str,
    after_uid: u32,
) -> anyhow::Result<(Vec<EmailSummary>, u32)> {
    let mut s = connect(acc, password).await?;
    s.examine("INBOX").await?;
    let all: Vec<u32> = s.uid_search("ALL").await?.into_iter().collect();
    let max = all.iter().copied().max().unwrap_or(after_uid);
    if after_uid == 0 {
        let _ = s.logout().await;
        return Ok((vec![], max));
    }
    let fresh: Vec<u32> = all.into_iter().filter(|u| *u > after_uid).collect();
    let out = fetch_uids(&mut s, fresh, 10).await;
    let _ = s.logout().await;
    Ok((out?, max.max(after_uid)))
}

pub struct Outgoing<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
    pub cc: Option<&'a str>,
    pub in_reply_to: Option<&'a str>,
}

pub async fn send(acc: &EmailAccount, password: &str, mail: Outgoing<'_>) -> anyhow::Result<()> {
    use lettre::message::{header::ContentType, Mailbox};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let host = acc
        .smtp_host
        .as_deref()
        .filter(|h| !h.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("SMTP server is not set for {}", acc.address))?;
    let from: Mailbox = match acc.display_name.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(n) => format!("{n} <{}>", acc.address).parse()?,
        None => acc.address.parse()?,
    };
    let mut builder = Message::builder().from(from).subject(mail.subject);
    for to in mail
        .to
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder = builder.to(to.parse()?);
    }
    for cc in mail
        .cc
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        builder = builder.cc(cc.parse()?);
    }
    if let Some(id) = mail.in_reply_to.filter(|s| !s.is_empty()) {
        let id = format!("<{}>", id.trim_matches(['<', '>']));
        builder = builder.in_reply_to(id.clone()).references(id);
    }
    let message = builder
        .header(ContentType::TEXT_PLAIN)
        .body(mail.body.to_string())?;
    let creds = Credentials::new(acc.login().to_string(), password.to_string());
    let transport = if acc.smtp_port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
    }
    .port(acc.smtp_port)
    .credentials(creds)
    .timeout(Some(std::time::Duration::from_secs(30)))
    .build();
    transport.send(message).await?;
    Ok(())
}

/// Log in to both servers without sending anything.
pub async fn test(acc: &EmailAccount, password: &str) -> anyhow::Result<()> {
    let mut s = connect(acc, password).await?;
    let _ = s.logout().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_a_plain_message() {
        let raw = b"From: Ana <ana@example.com>\r\nSubject: Hola\r\nMessage-ID: <x1@example.com>\r\nDate: Fri, 16 Oct 2026 10:00:00 +0000\r\n\r\nNos vemos el lunes.\r\n";
        let s = summarize(7, raw, true);
        assert_eq!(s.from, "Ana <ana@example.com>");
        assert_eq!(s.subject, "Hola");
        assert_eq!(s.message_id.as_deref(), Some("x1@example.com"));
        assert!(s.text.contains("lunes"));
        assert!(s.render(false).starts_with("#7 [unread]"));
    }

    #[test]
    fn quotes_imap_strings() {
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
    }
}

/// One message from Composio's `GMAIL_FETCH_EMAILS`.
#[derive(Debug, Clone, PartialEq)]
pub struct GmailMessage {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body: String,
}

/// Read `GMAIL_FETCH_EMAILS` output defensively: field names vary between Composio versions.
pub fn parse_gmail_messages(data: &serde_json::Value) -> Vec<GmailMessage> {
    let pick = |m: &serde_json::Value, keys: &[&str]| -> String {
        keys.iter()
            .find_map(|k| m.pointer(k).and_then(serde_json::Value::as_str))
            .unwrap_or_default()
            .to_string()
    };
    data.get("messages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = pick(m, &["/messageId", "/id"]);
            (!id.is_empty()).then(|| GmailMessage {
                from: pick(m, &["/sender", "/from"]),
                subject: pick(m, &["/subject", "/preview/subject"]),
                date: pick(m, &["/messageTimestamp", "/date"]),
                body: pick(m, &["/messageText", "/preview/body", "/snippet"])
                    .chars()
                    .take(4000)
                    .collect(),
                id,
            })
        })
        .collect()
}

#[cfg(test)]
mod gmail_tests {
    #[test]
    fn reads_composio_gmail_output() {
        let data = serde_json::json!({ "messages": [
            { "messageId": "a1", "sender": "Ana <ana@x.com>", "subject": "Hola", "messageText": "Body", "messageTimestamp": "2026-10-17T10:00:00Z" },
            { "subject": "no id" }
        ]});
        let m = super::parse_gmail_messages(&data);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].from, "Ana <ana@x.com>");
        assert_eq!(m[0].body, "Body");
    }
}
