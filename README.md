# Memento

*When you need to remember something...write it on your arm.*

Memento is an MCP (Model Context Protocol) server that gives AI agents persistent, structured memory backed by SQLite. It replaces flat-file memory with a queryable store that supports full-text search, pagination, and multi-agent coordination through shared artifacts and an append-only event log.

## Architecture

Memento organizes agent knowledge into three data types, each backed by its own SQLite table:

**Memories** are long-lived knowledge that persists across sessions. Each memory has a type (`user`, `feedback`, `project`, or `reference`), optional project scoping, and tags for categorization. Memories are the primary knowledge store — things agents learn and need to recall later.

**Artifacts** are intermediate work products shared between agents within a task. They support optional TTL expiration, source agent tracking, and type classification. Use artifacts for plans, intermediate results, or any ephemeral data that agents need to coordinate on but don't need to keep forever.

**Events** are an append-only log for reactive agent coordination. Agents emit events (e.g., `task_started`, `analysis_complete`) that other agents can poll using a monotonic cursor. Events enable fan-out/fan-in patterns and loose coupling between agents.

### Search

Memory search uses SQLite FTS5 (full-text search), not substring matching. Multi-word queries like `"tokens compliance"` find memories containing both words anywhere in the key or content. An inverted index keeps search performance constant as the store grows.

### Pagination

All list and search operations return paginated results with cursor-based navigation. Default page size is 20. Responses include a `next_cursor` and `has_more` flag. Cursor pagination is stable under concurrent writes — no skipped or duplicated rows.

### Transports

Memento supports two MCP transports:

- **stdio** (default) — Claude Code spawns memento as a child process. One client per instance. No configuration beyond the settings entry.
- **Streamable HTTP** — Memento runs as a standalone HTTP server. Multiple clients connect to a shared instance. Session management is handled automatically.

Both transports expose identical tools. The tool layer is transport-agnostic.

## Tools

| Tool | Description |
|---|---|
| `write_memory` | Store or update a memory with type, project, and tags |
| `read_memory` | Read a single memory by key |
| `search_memories` | Full-text search with type, project, and tag filters |
| `list_memories` | List memories with filters and pagination |
| `delete_memory` | Delete a memory by key |
| `write_artifact` | Store an artifact with optional TTL and source agent |
| `read_artifact` | Read an artifact by key (respects expiry) |
| `list_artifacts` | List non-expired artifacts with filters |
| `append_event` | Append an event to the coordination log |
| `read_events` | Poll events by cursor ID with filters |
| `get_stats` | Summary counts by type and project across all tables |

## Building

```
cargo build --release
```

The binary is at `target/release/memento`.

## Running

### stdio (for Claude Code)

```
memento
```

Communicates over stdin/stdout. Logs to stderr. The database is created at `~/.config/claude-memory/memory.db` on first run.

### HTTP

```
memento --transport http --host 127.0.0.1 --port 8080
```

Serves MCP over streamable HTTP at `http://127.0.0.1:8080/mcp`. Graceful shutdown on ctrl-c.

### Options

```
--db-path <PATH>       Path to SQLite database (default: ~/.config/claude-memory/memory.db)
--transport <stdio|http> Transport protocol (default: stdio)
--host <ADDR>          Bind address for HTTP (default: 127.0.0.1)
--port <PORT>          Port for HTTP (default: 8080)
```

The database path can also be set via the `MEMENTO_DB_PATH` environment variable.

### Logging

Set `RUST_LOG` to control log verbosity:

```
RUST_LOG=memento=debug memento
```

## Configuring Claude Code

Add memento to your Claude Code settings file at `~/.claude/settings.json`:

### stdio transport

```json
{
  "mcpServers": {
    "memento": {
      "command": "/path/to/memento",
      "args": []
    }
  }
}
```

With a custom database path:

```json
{
  "mcpServers": {
    "memento": {
      "command": "/path/to/memento",
      "args": ["--db-path", "/path/to/memory.db"]
    }
  }
}
```

### HTTP transport

Start the server separately, then point Claude Code at it:

```json
{
  "mcpServers": {
    "memento": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

## Stack

- **Rust** with tokio async runtime
- **SQLite** via sqlx (with FTS5 full-text search)
- **rmcp** — official Rust MCP SDK
- **axum** — HTTP transport server
