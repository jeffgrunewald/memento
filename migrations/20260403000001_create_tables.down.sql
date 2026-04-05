-- Reverse table creation in dependency order
DROP INDEX IF EXISTS idx_events_created_at;
DROP INDEX IF EXISTS idx_events_event_type;
DROP INDEX IF EXISTS idx_events_project;
DROP TABLE IF EXISTS events;

DROP INDEX IF EXISTS idx_artifacts_created_at;
DROP INDEX IF EXISTS idx_artifacts_artifact_type;
DROP INDEX IF EXISTS idx_artifacts_project;
DROP TABLE IF EXISTS artifacts;

DROP INDEX IF EXISTS idx_memories_created_at;
DROP INDEX IF EXISTS idx_memories_memory_type;
DROP INDEX IF EXISTS idx_memories_project;
DROP TABLE IF EXISTS memories;
