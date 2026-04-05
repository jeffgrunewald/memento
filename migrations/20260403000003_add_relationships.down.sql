-- Remove knowledge graph edges
DROP INDEX IF EXISTS idx_relationships_relation;
DROP INDEX IF EXISTS idx_relationships_target;
DROP INDEX IF EXISTS idx_relationships_source;
DROP TABLE IF EXISTS relationships;
