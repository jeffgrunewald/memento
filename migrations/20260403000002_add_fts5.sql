-- FTS5 virtual table for full-text search on memories
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    key,
    content,
    content=memories,
    content_rowid=rowid
);

-- Populate FTS index from existing data
INSERT INTO memories_fts(rowid, key, content)
    SELECT rowid, key, content FROM memories;

-- Keep FTS in sync with the memories table
CREATE TRIGGER memories_fts_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, key, content)
        VALUES (new.rowid, new.key, new.content);
END;

CREATE TRIGGER memories_fts_update AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, content)
        VALUES ('delete', old.rowid, old.key, old.content);
    INSERT INTO memories_fts(rowid, key, content)
        VALUES (new.rowid, new.key, new.content);
END;

CREATE TRIGGER memories_fts_delete AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, key, content)
        VALUES ('delete', old.rowid, old.key, old.content);
END;
