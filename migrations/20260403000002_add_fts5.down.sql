-- Remove FTS5 sync triggers and virtual table
DROP TRIGGER IF EXISTS memories_fts_delete;
DROP TRIGGER IF EXISTS memories_fts_update;
DROP TRIGGER IF EXISTS memories_fts_insert;
DROP TABLE IF EXISTS memories_fts;
