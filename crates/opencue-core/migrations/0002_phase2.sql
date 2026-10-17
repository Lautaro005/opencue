-- Phase 2: drawn avatars, automations and per-agent email accounts.
ALTER TABLE agents RENAME COLUMN avatar_emoji TO avatar_face;

CREATE TABLE IF NOT EXISTS automations (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    trigger_kind  TEXT NOT NULL,              -- schedule | email | webhook | group_message
    trigger       TEXT NOT NULL DEFAULT '{}', -- JSON, shape depends on trigger_kind
    prompt        TEXT NOT NULL DEFAULT '',
    enabled       INTEGER NOT NULL DEFAULT 1,
    last_run_at   TEXT,
    last_status   TEXT,
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_automations_agent ON automations(agent_id);

CREATE TABLE IF NOT EXISTS email_accounts (
    agent_id      TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL DEFAULT 'imap', -- imap | composio_gmail
    address       TEXT NOT NULL,
    display_name  TEXT,
    imap_host     TEXT,
    imap_port     INTEGER NOT NULL DEFAULT 993,
    smtp_host     TEXT,
    smtp_port     INTEGER NOT NULL DEFAULT 465,
    username      TEXT,
    last_seen_uid INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL
);
