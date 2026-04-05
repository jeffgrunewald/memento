-- Directed edges between memories and artifacts for knowledge graph traversal
CREATE TABLE IF NOT EXISTS relationships (
    source_type TEXT NOT NULL CHECK (source_type IN ('memory', 'artifact')),
    source_key  TEXT NOT NULL,
    target_type TEXT NOT NULL CHECK (target_type IN ('memory', 'artifact')),
    target_key  TEXT NOT NULL,
    relation    TEXT NOT NULL,  -- e.g. "relates_to", "depends_on", "supersedes", "derived_from"
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (source_type, source_key, target_type, target_key, relation)
);

CREATE INDEX idx_relationships_source ON relationships(source_type, source_key);
CREATE INDEX idx_relationships_target ON relationships(target_type, target_key);
CREATE INDEX idx_relationships_relation ON relationships(relation);
