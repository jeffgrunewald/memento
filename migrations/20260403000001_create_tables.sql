-- Memories: long-lived knowledge that persists across sessions
CREATE TABLE IF NOT EXISTS memories (
    key         TEXT PRIMARY KEY,
    content     TEXT NOT NULL,
    memory_type TEXT NOT NULL CHECK (memory_type IN ('user', 'feedback', 'project', 'reference')),
    project     TEXT,  -- NULL = global knowledge
    tags        TEXT NOT NULL DEFAULT '[]',  -- JSON array of strings
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_memories_project ON memories(project);
CREATE INDEX idx_memories_memory_type ON memories(memory_type);
CREATE INDEX idx_memories_created_at ON memories(created_at);

-- Artifacts: intermediate work products shared between agents within a task
CREATE TABLE IF NOT EXISTS artifacts (
    key            TEXT PRIMARY KEY,
    content        TEXT NOT NULL,
    artifact_type  TEXT NOT NULL,
    project        TEXT,
    source_agent   TEXT,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at     TEXT  -- NULL = no expiry
);

CREATE INDEX idx_artifacts_project ON artifacts(project);
CREATE INDEX idx_artifacts_artifact_type ON artifacts(artifact_type);
CREATE INDEX idx_artifacts_created_at ON artifacts(created_at);

-- Events: append-only log for reactive agent coordination
CREATE TABLE IF NOT EXISTS events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    source_agent TEXT NOT NULL,
    event_type   TEXT NOT NULL,
    payload      TEXT NOT NULL DEFAULT '{}',  -- JSON object
    project      TEXT,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_events_project ON events(project);
CREATE INDEX idx_events_event_type ON events(event_type);
CREATE INDEX idx_events_created_at ON events(created_at);
