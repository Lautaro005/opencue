//! Messenger (Settings → Messenger): WhatsApp for an agent. The app links to the user's WhatsApp
//! as a companion device (scan a QR in Linked devices) and runs the connection itself, so nothing
//! is hosted anywhere. A message that arrives is answered by the chosen agent in its own
//! conversation with that contact, and the exchange is noted in the agent's chat as a green card.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use whatsapp_rust::bot::BotHandle;
use whatsapp_rust::prelude::*;
use whatsapp_rust::wacore_binary::JidExt;

use crate::events::CoreEvent;
use crate::models::{Agent, Role};
use crate::runtime::message;
use crate::Core;

const AGENT_KEY: &str = "whatsapp_agent";
const ALLOW_KEY: &str = "whatsapp_allow";
const ENABLED_KEY: &str = "whatsapp_enabled";

/// The conversation an agent has with one WhatsApp contact (kept apart from the others and from
/// the agent's chat with its owner).
pub fn wa_thread_id(agent_id: &str, phone: &str) -> String {
    format!("wa_{agent_id}_{phone}")
}

pub fn is_wa_thread(thread_id: &str) -> bool {
    thread_id.starts_with("wa_")
}

/// The note an exchange leaves in the agent's own chat; the UI shows it as a WhatsApp card.
pub fn exchange_note(contact: &str, question: &str, answer: &str) -> String {
    format!("WhatsApp · {contact}\n\nQ: {question}\n\nA: {answer}")
}

/// Whether a sender may talk to the agent: everyone when the list is empty, else the numbers
/// listed (comma or space separated, digits only compared, a country-code prefix is enough).
pub fn allowed(list: &str, phone: &str) -> bool {
    let digits = |s: &str| s.chars().filter(char::is_ascii_digit).collect::<String>();
    let wanted: Vec<String> = list
        .split([',', ';', ' ', '\n'])
        .map(digits)
        .filter(|d| !d.is_empty())
        .collect();
    if wanted.is_empty() {
        return true;
    }
    let phone = digits(phone);
    wanted
        .iter()
        .any(|w| phone == *w || phone.ends_with(w.as_str()) || w.ends_with(phone.as_str()))
}

#[derive(Default)]
pub(crate) struct WaRuntime {
    state: String,
    qr_svg: Option<String>,
    phone: Option<String>,
    error: Option<String>,
    handle: Option<BotHandle>,
    /// One conversation at a time per contact.
    locks: HashMap<String, Arc<tokio::sync::Mutex<()>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessengerStatus {
    /// `off`, `starting`, `qr` (waiting for the scan), `connected` or `error`.
    pub state: String,
    pub qr_svg: Option<String>,
    pub phone: Option<String>,
    pub agent_id: Option<String>,
    pub allow: String,
    pub error: Option<String>,
}

fn qr_svg(code: &str) -> Option<String> {
    let qr = qrcode::QrCode::new(code.as_bytes()).ok()?;
    Some(
        qr.render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

impl Core {
    fn wa_db(&self) -> std::path::PathBuf {
        self.paths().root().join("messenger").join("whatsapp.db")
    }

    pub async fn messenger_status(&self) -> MessengerStatus {
        let (state, qr, phone, error) = {
            let w = self.wa.lock().unwrap();
            (
                if w.state.is_empty() {
                    "off".to_string()
                } else {
                    w.state.clone()
                },
                w.qr_svg.clone(),
                w.phone.clone(),
                w.error.clone(),
            )
        };
        MessengerStatus {
            state,
            qr_svg: qr,
            phone,
            agent_id: self
                .store
                .setting(AGENT_KEY)
                .await
                .ok()
                .flatten()
                .filter(|s| !s.is_empty()),
            allow: self
                .store
                .setting(ALLOW_KEY)
                .await
                .ok()
                .flatten()
                .unwrap_or_default(),
            error,
        }
    }

    fn wa_update(&self, f: impl FnOnce(&mut WaRuntime)) {
        f(&mut self.wa.lock().unwrap());
        self.emit(CoreEvent::MessengerChanged);
    }

    /// Choose the agent that answers and who may write to it.
    pub async fn whatsapp_configure(
        &self,
        agent_id: Option<String>,
        allow: String,
    ) -> anyhow::Result<()> {
        if let Some(id) = &agent_id {
            self.store.get_agent(id).await?;
        }
        self.store
            .set_setting(AGENT_KEY, agent_id.as_deref().unwrap_or(""))
            .await?;
        self.store.set_setting(ALLOW_KEY, allow.trim()).await?;
        self.emit(CoreEvent::MessengerChanged);
        Ok(())
    }

    /// Start linking: a QR appears in Settings → Messenger. Linked before, it reconnects.
    pub async fn whatsapp_connect(self: &Arc<Self>) -> anyhow::Result<()> {
        anyhow::ensure!(self.primary, "WhatsApp runs in the desktop app");
        let has_agent = self
            .store
            .setting(AGENT_KEY)
            .await?
            .is_some_and(|s| !s.is_empty());
        anyhow::ensure!(has_agent, "choose the agent that will answer first");
        self.store.set_setting(ENABLED_KEY, "true").await?;
        self.wa_start();
        Ok(())
    }

    /// Unlink the phone and forget the session.
    pub async fn whatsapp_disconnect(self: &Arc<Self>) -> anyhow::Result<()> {
        let handle = self.wa.lock().unwrap().handle.take();
        if let Some(h) = handle {
            let client = h.client();
            client.logout().await;
            h.shutdown().await;
        }
        self.store.set_setting(ENABLED_KEY, "false").await?;
        if let Some(dir) = self.wa_db().parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
        self.wa_update(|w| *w = WaRuntime::default());
        Ok(())
    }

    /// At launch: reconnect a WhatsApp that was linked.
    pub(crate) fn wa_autostart(self: &Arc<Self>) {
        let core = self.clone();
        tokio::spawn(async move {
            if core
                .store
                .setting(ENABLED_KEY)
                .await
                .ok()
                .flatten()
                .as_deref()
                == Some("true")
            {
                core.wa_start();
            }
        });
    }

    fn wa_start(self: &Arc<Self>) {
        {
            let mut w = self.wa.lock().unwrap();
            if w.handle.is_some() || w.state == "starting" {
                return;
            }
            w.state = "starting".into();
            w.error = None;
        }
        self.emit(CoreEvent::MessengerChanged);
        let core = self.clone();
        tokio::spawn(async move {
            if let Err(e) = core.wa_run().await {
                core.wa_update(|w| {
                    w.state = "error".into();
                    w.error = Some(format!("{e:#}"));
                });
            }
        });
    }

    async fn wa_run(self: &Arc<Self>) -> anyhow::Result<()> {
        let db = self.wa_db();
        if let Some(dir) = db.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let store = SqliteStore::new(&db.display().to_string())
            .await
            .map_err(|e| anyhow::anyhow!("could not open the WhatsApp session: {e}"))?;
        let (c_qr, c_ok, c_out, c_msg) = (self.clone(), self.clone(), self.clone(), self.clone());
        let bot = Bot::builder()
            .with_backend(store)
            .on_qr_code(move |code, _| {
                let core = c_qr.clone();
                async move {
                    core.wa_update(|w| {
                        w.state = "qr".into();
                        w.qr_svg = qr_svg(&code);
                    });
                }
            })
            .on_connected(move |client| {
                let core = c_ok.clone();
                async move {
                    let phone = client.pn().map(|j| j.user.to_string());
                    core.wa_update(|w| {
                        w.state = "connected".into();
                        w.qr_svg = None;
                        w.phone = phone;
                        w.error = None;
                    });
                }
            })
            .on_logged_out(move |_| {
                let core = c_out.clone();
                async move {
                    let _ = core.store.set_setting(ENABLED_KEY, "false").await;
                    if let Some(dir) = core.wa_db().parent() {
                        let _ = std::fs::remove_dir_all(dir);
                    }
                    core.wa_update(|w| {
                        w.handle = None;
                        w.state = "off".into();
                        w.qr_svg = None;
                        w.phone = None;
                    });
                }
            })
            .on_message(move |ctx| {
                let core = c_msg.clone();
                async move { core.wa_incoming(ctx).await }
            })
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("could not start WhatsApp: {e}"))?;
        let handle = bot.spawn();
        self.wa.lock().unwrap().handle = Some(handle);
        Ok(())
    }

    /// A WhatsApp message arrived: the agent answers it.
    async fn wa_incoming(self: &Arc<Self>, ctx: MessageContext) {
        let src = &ctx.info.source;
        if src.is_from_me
            || src.is_group
            || src.chat.is_group()
            || src.chat.is_status_broadcast()
            || src.chat.is_newsletter()
        {
            return;
        }
        let Some(text) = ctx.message.text_content().map(|t| t.trim().to_string()) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        // Newer accounts show a contact as an opaque id; prefer their phone number when WhatsApp gives it.
        let sender = if src.sender.to_string().ends_with("@lid") {
            src.sender_alt.clone().unwrap_or_else(|| src.sender.clone())
        } else {
            src.sender.clone()
        };
        let phone = sender.user.to_string();
        let allow = self
            .store
            .setting(ALLOW_KEY)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        if !allowed(&allow, &phone) {
            return;
        }
        let agent: Agent = match self.store.setting(AGENT_KEY).await.ok().flatten() {
            Some(id) if !id.is_empty() => match self.store.get_agent(&id).await {
                Ok(a) => a,
                Err(_) => return,
            },
            _ => return,
        };
        let lock = self
            .wa
            .lock()
            .unwrap()
            .locks
            .entry(phone.clone())
            .or_default()
            .clone();
        let _turn = lock.lock().await;

        let thread = wa_thread_id(&agent.id, &phone);
        let msg = message(&thread, Role::User, None, text.clone());
        if self.store.insert_message(&msg).await.is_err() {
            return;
        }
        self.emit(CoreEvent::Message { message: msg });
        let outcome = self
            .run_agent(&thread, &agent.id, None, &CancellationToken::new())
            .await;
        let reply = outcome.text.trim().to_string();
        if reply.is_empty() {
            return;
        }
        if let Err(e) = ctx.reply(reply.clone()).await {
            tracing::warn!("whatsapp reply failed: {e}");
        }
        // The exchange, as a card in the agent's own chat.
        let note = message(
            &agent.id,
            Role::Event,
            None,
            exchange_note(&format!("+{phone}"), &text, &reply),
        );
        if self.store.insert_message(&note).await.is_ok() {
            self.emit(CoreEvent::Message { message: note });
            self.emit(CoreEvent::ThreadsChanged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allow_list_matches_numbers_by_their_digits() {
        assert!(allowed("", "5491155550000"));
        assert!(allowed("+54 9 11 5555 0000", "5491155550000"));
        assert!(allowed("5491155550000, 123", "5491155550000"));
        assert!(allowed("91155550000", "5491155550000"));
        assert!(!allowed("5491100000000", "5491155550000"));
        // No digits means no restriction.
        assert!(allowed("abc", "5491155550000"));
    }

    #[test]
    fn exchanges_have_a_stable_shape() {
        let n = exchange_note("+54911", "hola?", "hola!");
        assert_eq!(n, "WhatsApp · +54911\n\nQ: hola?\n\nA: hola!");
        assert!(is_wa_thread(&wa_thread_id("a1", "54911")));
        assert!(!is_wa_thread("dm_a_b"));
    }
}
