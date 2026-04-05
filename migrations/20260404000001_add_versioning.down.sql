-- Remove versioning: trigger, history table, and version column
DROP TRIGGER IF EXISTS memory_version_history;
DROP INDEX IF EXISTS idx_memory_versions_key_version;
DROP INDEX IF EXISTS idx_memory_versions_key;
DROP TABLE IF EXISTS memory_versions;
ALTER TABLE memories DROP COLUMN version;
