use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{artifact, event, memory, schema::MemoryType, stats};

#[derive(Clone)]
pub struct MementoServer {
    pool: SqlitePool,
    tool_router: ToolRouter<Self>,
}

impl MementoServer {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }
}

// -- Tool input types --

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteMemoryInput {
    /// Unique key identifying this memory
    pub key: String,
    /// The memory content (text or structured data)
    pub content: String,
    /// Type of memory: user, feedback, project, or reference
    pub memory_type: String,
    /// Project scope (omit for global memories)
    pub project: Option<String>,
    /// JSON array of tag strings for categorization
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMemoryInput {
    /// Key of the memory to read
    pub key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchMemoriesInput {
    /// Text to search for in memory keys and content
    pub query: String,
    /// Filter by memory type: user, feedback, project, or reference
    pub memory_type: Option<String>,
    /// Filter by project scope
    pub project: Option<String>,
    /// Filter by tag (exact match against the tags array)
    pub tag: Option<String>,
    /// Cursor from a previous response for pagination
    pub cursor: Option<String>,
    /// Maximum number of results to return (default: 20)
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListMemoriesInput {
    /// Filter by memory type: user, feedback, project, or reference
    pub memory_type: Option<String>,
    /// Filter by project scope
    pub project: Option<String>,
    /// Filter by tag (exact match against the tags array)
    pub tag: Option<String>,
    /// Cursor from a previous response for pagination
    pub cursor: Option<String>,
    /// Maximum number of results to return (default: 20)
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteMemoryInput {
    /// Key of the memory to delete
    pub key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteArtifactInput {
    /// Unique key identifying this artifact
    pub key: String,
    /// The artifact content
    pub content: String,
    /// Type of artifact (e.g. plan, result, intermediate)
    pub artifact_type: String,
    /// Project scope
    pub project: Option<String>,
    /// Name of the agent that produced this artifact
    pub source_agent: Option<String>,
    /// ISO 8601 expiration timestamp (omit for no expiry)
    pub expires_at: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadArtifactInput {
    /// Key of the artifact to read
    pub key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArtifactsInput {
    /// Filter by artifact type
    pub artifact_type: Option<String>,
    /// Filter by project scope
    pub project: Option<String>,
    /// Cursor from a previous response for pagination
    pub cursor: Option<String>,
    /// Maximum number of results to return (default: 20)
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendEventInput {
    /// Name of the agent emitting this event
    pub source_agent: String,
    /// Event type identifier
    pub event_type: String,
    /// JSON payload for the event
    pub payload: Option<String>,
    /// Project scope
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadEventsInput {
    /// Event ID cursor — return events with id greater than this value (omit to start from beginning)
    pub after_id: Option<i64>,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by project scope
    pub project: Option<String>,
    /// Maximum number of results to return (default: 20)
    pub limit: Option<i64>,
}

// -- Tool implementations --

fn tool_error(msg: impl std::fmt::Display) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(msg.to_string())]))
}

fn tool_success(value: &impl serde::Serialize) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(
        rmcp::serde_json::to_string_pretty(value).unwrap_or_default(),
    )]))
}

#[tool_router]
impl MementoServer {
    #[tool(
        description = "Store or update a memory. Memories are long-lived knowledge that persists across sessions."
    )]
    async fn write_memory(
        &self,
        Parameters(input): Parameters<WriteMemoryInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(memory_type) = MemoryType::parse(&input.memory_type) else {
            return tool_error(format!(
                "invalid memory_type '{}': must be user, feedback, project, or reference",
                input.memory_type
            ));
        };
        let tags = input.tags.as_deref().unwrap_or("[]");
        match memory::write(
            &self.pool,
            &input.key,
            &input.content,
            memory_type,
            input.project.as_deref(),
            tags,
        )
        .await
        {
            Ok(row) => tool_success(&row),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Read a memory by its key.")]
    async fn read_memory(
        &self,
        Parameters(input): Parameters<ReadMemoryInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match memory::read(&self.pool, &input.key).await {
            Ok(Some(row)) => tool_success(&row),
            Ok(None) => tool_error(format!("memory '{}' not found", input.key)),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Search memories by text query with optional type and project filters. Returns paginated results."
    )]
    async fn search_memories(
        &self,
        Parameters(input): Parameters<SearchMemoriesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let memory_type = input.memory_type.as_deref().and_then(MemoryType::parse);
        match memory::search(
            &self.pool,
            &input.query,
            memory_type,
            input.project.as_deref(),
            input.tag.as_deref(),
            input.cursor.as_deref(),
            input.limit,
        )
        .await
        {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "List memories with optional type and project filters. Returns paginated results."
    )]
    async fn list_memories(
        &self,
        Parameters(input): Parameters<ListMemoriesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let memory_type = input.memory_type.as_deref().and_then(MemoryType::parse);
        match memory::list(
            &self.pool,
            memory_type,
            input.project.as_deref(),
            input.tag.as_deref(),
            input.cursor.as_deref(),
            input.limit,
        )
        .await
        {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Delete a memory by its key. Returns whether the memory existed.")]
    async fn delete_memory(
        &self,
        Parameters(input): Parameters<DeleteMemoryInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match memory::delete(&self.pool, &input.key).await {
            Ok(deleted) => tool_success(&rmcp::serde_json::json!({ "deleted": deleted })),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Store or update an artifact. Artifacts are intermediate work products shared between agents within a task."
    )]
    async fn write_artifact(
        &self,
        Parameters(input): Parameters<WriteArtifactInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let expires_at = match input.expires_at.as_deref() {
            Some(s) => {
                match time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
                {
                    Ok(dt) => Some(dt),
                    Err(e) => return tool_error(format!("invalid expires_at: {e}")),
                }
            }
            None => None,
        };
        match artifact::write(
            &self.pool,
            &input.key,
            &input.content,
            &input.artifact_type,
            input.project.as_deref(),
            input.source_agent.as_deref(),
            expires_at,
        )
        .await
        {
            Ok(row) => tool_success(&row),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Read an artifact by its key. Expired artifacts are not returned.")]
    async fn read_artifact(
        &self,
        Parameters(input): Parameters<ReadArtifactInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match artifact::read(&self.pool, &input.key).await {
            Ok(Some(row)) => tool_success(&row),
            Ok(None) => tool_error(format!("artifact '{}' not found or expired", input.key)),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "List artifacts with optional type and project filters. Expired artifacts are excluded. Returns paginated results."
    )]
    async fn list_artifacts(
        &self,
        Parameters(input): Parameters<ListArtifactsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match artifact::list(
            &self.pool,
            input.artifact_type.as_deref(),
            input.project.as_deref(),
            input.cursor.as_deref(),
            input.limit,
        )
        .await
        {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Append an event to the log. Events are immutable records for reactive agent coordination."
    )]
    async fn append_event(
        &self,
        Parameters(input): Parameters<AppendEventInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let payload = input.payload.as_deref().unwrap_or("{}");
        match event::append(
            &self.pool,
            &input.source_agent,
            &input.event_type,
            payload,
            input.project.as_deref(),
        )
        .await
        {
            Ok(row) => tool_success(&row),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Get summary statistics: total counts of memories, artifacts, and events, broken down by type and project. Use this to orient before querying."
    )]
    async fn get_stats(&self) -> Result<CallToolResult, ErrorData> {
        match stats::get_stats(&self.pool).await {
            Ok(s) => tool_success(&s),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "Read events from the log. Use after_id for cursor-based polling — pass the last seen event id to get only newer events. Returns paginated results."
    )]
    async fn read_events(
        &self,
        Parameters(input): Parameters<ReadEventsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match event::read_since(
            &self.pool,
            input.after_id,
            input.event_type.as_deref(),
            input.project.as_deref(),
            input.limit,
        )
        .await
        {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }
}

#[tool_handler]
impl ServerHandler for MementoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Memento: shared memory server for multi-agent coordination. \
                 Provides persistent memories, ephemeral artifacts, and an append-only event log."
                .to_string(),
        )
    }
}
