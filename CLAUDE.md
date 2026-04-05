# Memento MCP Server

Rust MCP server providing persistent memory, ephemeral artifacts, an event log, and a knowledge graph for multi-agent coordination. SQLite-backed with FTS5 full-text search.

## Build & Run

```bash
cargo build --release
cargo install --path .
# stdio (default): memento
# HTTP: memento serve --transport http --port 8080
```

## Architecture

Four data types, each with its own access pattern:

- **memories** — long-lived versioned knowledge (user, feedback, project, reference types). Keyed, searchable via FTS5. Scoped by project, tagged for categorization.
- **artifacts** — ephemeral work products between agents. Optional TTL via `expires_at`. Scoped by project and source agent.
- **events** — append-only coordination log. Monotonic ID cursor for reliable polling. Never mutated.
- **relationships** — directed edges between memories and artifacts, forming a knowledge graph.

DB is at `~/.config/claude-memory/memory.db` by default. Override with `--db-path` or `MEMENTO_DB_PATH`.

## Tool Usage Guide

### Orientation

Always call `get_stats` first when connecting to an existing database. It returns counts by type and project across all three tables — lets you know what's there before pulling content.

### Token efficiency

`list_memories`, `search_memories`, and `list_artifacts` return **summaries by default** — key, metadata, and a 100-character content preview. This saves tokens when browsing. Use `read_memory` or `read_artifact` to get full content for specific items. Pass `full: true` only when you need complete content for an entire page.

### Memories

**Key naming**: Keys must be descriptive and specific enough to avoid unintentional collisions. A key is a unique identifier — two memories with the same key are treated as versions of the same knowledge, so keys that are too generic will cause unrelated content to overwrite each other.

- Good: `auth-middleware-compliance-rewrite`, `solana-rpc-commitment-levels`, `jeff-prefers-functional-patterns`
- Bad: `auth`, `config`, `notes`, `todo`

When scoped to a project, include enough context to distinguish from similar concepts: `memento/versioning-design-decisions` rather than just `versioning`.

Use `memory_type` consistently:
- `user` — who the user is, their preferences, expertise
- `feedback` — corrections and confirmations about how to work
- `project` — ongoing work, goals, decisions, deadlines
- `reference` — pointers to external systems and resources

Write with tags for later filtering: `tags: '["rust", "architecture"]'`. Tags must be a valid JSON array of strings.

Tag conventions for scoping:
- `agent:<name>` — filter by agent role when needed (e.g. `agent:architect`). Note: agent role names are not stable across projects; don't assume other projects use the same names.
- `task:<id>` — scope artifacts and memories to a specific task for fan-out/fan-in coordination. Orchestrator generates the ID and passes it to sub-agents.

`search_memories` uses FTS5 — multi-word queries find memories containing all terms. Prefer search over list when you know what you're looking for.

### Versioning

Every memory has an integer `version`. Use version 1 for new memories. To update an existing memory, read it first and write back with `version: current + 1`. If another agent updated the memory since your read, the write returns a conflict with the current version and a content preview — resolve and retry.

Use `get_history` to view previous versions of a memory.

### Batch writes

`write_memories` and `write_artifacts` accept arrays. Each item is processed independently — the response contains per-item results. Items that succeed return `status: "ok"`, items that conflict return `status: "conflict"` with the current version. Retry only the conflicting items after resolving.

### Artifacts

Set `expires_at` (RFC 3339 timestamp) for temporary work products. Expired artifacts are filtered from reads automatically.

Include `source_agent` when writing so other agents know who produced the artifact.

### Events

Use `append_event` to signal state changes. Use `read_events` with `after_id` to poll — pass the last seen event ID to get only new events. This is the coordination primitive for fan-out/fan-in patterns.

### Relationships (knowledge graph)

Use `link` to create directed, typed relationships between memories and/or artifacts. Use `get_related` to traverse the graph in both directions (outgoing and incoming edges).

Relationship type conventions:
- `relates_to` — general association between two concepts
- `depends_on` — this entity requires the target to be understood/completed first
- `supersedes` — this entity replaces or updates the target
- `derived_from` — this entity was produced using the target as input

Use `unlink` to remove relationships. Omit the `relation` parameter to remove all relationships between a pair.

### Pagination

All list and search operations are paginated (default: 20 items, max: 100). Response includes `has_more` and `next_cursor`. Pass `cursor` back to get the next page. Don't request more than you need — use `limit` to control page size.

## Development

```bash
cargo check                           # type check
cargo clippy -- -D warnings           # lint
cargo test                            # run tests
cargo build                           # debug build
RUST_LOG=memento=debug cargo run      # run with debug logging
```

SQLite migrations are embedded in the binary and run automatically on startup.

## Project Structure

```
src/
  main.rs          — CLI, transport setup, shutdown signal
  db.rs            — connection pool, migration runner
  schema.rs        — row types, pagination trait/helpers, ContentDetail enum
  memory.rs        — memory CRUD + FTS5 search + versioning
  artifact.rs      — artifact CRUD with TTL filtering
  event.rs         — append-only event log with ID cursor
  relationship.rs  — knowledge graph edges (link, unlink, traverse)
  server.rs        — MCP tool definitions (17 tools via rmcp handlers)
  stats.rs         — summary statistics queries
  import.rs        — multi-framework memory importer
migrations/        — SQLite schema (auto-applied)
```
