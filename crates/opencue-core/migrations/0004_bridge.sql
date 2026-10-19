-- Requests that cross the process boundary: the tool server of a CLI-backed agent (a separate
-- process) asks the running app to question the user or to talk to another agent.
CREATE TABLE IF NOT EXISTS bridge (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,            -- ask | agent
    agent_id   TEXT NOT NULL,
    thread_id  TEXT,
    payload    TEXT NOT NULL DEFAULT '{}',
    status     TEXT NOT NULL DEFAULT 'pending', -- pending | announced | working | done | cancelled
    result     TEXT,
    created_at TEXT NOT NULL
);
