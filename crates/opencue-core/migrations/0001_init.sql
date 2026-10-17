-- OpenCue initial schema. Long-form text (instructions, memory, skills) lives in Markdown files.
CREATE TABLE IF NOT EXISTS providers (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    name          TEXT NOT NULL,
    base_url      TEXT,
    default_model TEXT,
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agents (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    avatar_emoji  TEXT NOT NULL DEFAULT '',
    avatar_color  TEXT NOT NULL DEFAULT '#7c8cff',
    avatar_shape  TEXT NOT NULL DEFAULT 'circle',
    provider_id   TEXT REFERENCES providers(id) ON DELETE SET NULL,
    model         TEXT,
    settings      TEXT NOT NULL DEFAULT '{}',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS groups (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    max_auto_turns  INTEGER NOT NULL DEFAULT 6,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS group_members (
    group_id   TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    is_owner   INTEGER NOT NULL DEFAULT 0,
    position   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (group_id, agent_id)
);

-- A thread is either an agent's 1:1 chat (thread_id = agent id) or a group (thread_id = group id).
CREATE TABLE IF NOT EXISTS messages (
    id            TEXT PRIMARY KEY,
    thread_id     TEXT NOT NULL,
    role          TEXT NOT NULL,          -- user | assistant | tool | event
    agent_id      TEXT,                   -- author agent for assistant/tool messages
    content       TEXT NOT NULL DEFAULT '',
    tool_calls    TEXT,                   -- JSON array of ToolCall (assistant)
    tool_call_id  TEXT,                   -- tool result link
    tool_name     TEXT,
    status        TEXT NOT NULL DEFAULT 'done', -- done | error | denied | pending
    input_tokens  INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id, created_at);

CREATE TABLE IF NOT EXISTS mcp_servers (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE,
    transport  TEXT NOT NULL,             -- stdio | http
    command    TEXT,
    args       TEXT NOT NULL DEFAULT '[]',
    env        TEXT NOT NULL DEFAULT '{}',
    url        TEXT,
    headers    TEXT NOT NULL DEFAULT '{}',
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS web_cache (
    key        TEXT PRIMARY KEY,
    body       TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);
