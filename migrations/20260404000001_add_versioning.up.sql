-- Add version column to memories
ALTER TABLE memories ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

-- Version history table — populated by trigger on UPDATE
CREATE TABLE IF NOT EXISTS memory_versions (
    version_id    INTEGER PRIMARY KEY AUTOINCREMENT,
    key           TEXT NOT NULL,
    version       INTEGER NOT NULL,
    content       TEXT NOT NULL,
    memory_type   TEXT NOT NULL,
    project       TEXT,
    tags          TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    superseded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_memory_versions_key ON memory_versions(key);
CREATE INDEX idx_memory_versions_key_version ON memory_versions(key, version);

-- Trigger: snapshot current row into history before UPDATE
CREATE TRIGGER memory_version_history BEFORE UPDATE ON memories
BEGIN
    INSERT INTO memory_versions (key, version, content, memory_type, project, tags, created_at, updated_at)
    VALUES (old.key, old.version, old.content, old.memory_type, old.project, old.tags, old.created_at, old.updated_at);
END;
