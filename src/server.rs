use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::schema::{ContentDetail, MemoryType};
use crate::util::compact_content;
use crate::{artifact, event, memory, relationship, stats};

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
    /// Version number. Use 1 for new memories. For updates, increment the version from the last read by 1. A version mismatch returns a conflict with the current state.
    pub version: i64,
    /// Project scope (omit for global memories)
    pub project: Option<String>,
    /// JSON array of tag strings for categorization
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetHistoryInput {
    /// Key of the memory to get history for
    pub key: String,
    /// Maximum number of versions to return (default: 20)
    pub limit: Option<i64>,
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
    /// Return full content instead of preview (default: false). Omit to get summaries, which saves tokens. Use read_memory/read_artifact for full content of specific items.
    pub full: Option<bool>,
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
    /// Return full content instead of preview (default: false). Omit to get summaries, which saves tokens. Use read_memory/read_artifact for full content of specific items.
    pub full: Option<bool>,
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
    /// Return full content instead of preview (default: false). Omit to get summaries, which saves tokens. Use read_memory/read_artifact for full content of specific items.
    pub full: Option<bool>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteMemoriesBatchInput {
    /// Array of memories to write
    pub memories: Vec<WriteMemoryInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteArtifactItem {
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
pub struct WriteArtifactsBatchInput {
    /// Array of artifacts to write
    pub artifacts: Vec<WriteArtifactItem>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkInput {
    /// Source entity type: "memory" or "artifact"
    pub source_type: String,
    /// Key of the source entity
    pub source_key: String,
    /// Target entity type: "memory" or "artifact"
    pub target_type: String,
    /// Key of the target entity
    pub target_key: String,
    /// Relationship type (e.g. "relates_to", "depends_on", "supersedes", "derived_from")
    pub relation: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnlinkInput {
    /// Source entity type: "memory" or "artifact"
    pub source_type: String,
    /// Key of the source entity
    pub source_key: String,
    /// Target entity type: "memory" or "artifact"
    pub target_type: String,
    /// Key of the target entity
    pub target_key: String,
    /// Relationship type to remove (omit to remove all relationships between the pair)
    pub relation: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRelatedInput {
    /// Entity type: "memory" or "artifact"
    pub entity_type: String,
    /// Key of the entity
    pub key: String,
    /// Filter by relationship type
    pub relation: Option<String>,
}

// -- Helpers --

fn tool_error(msg: impl std::fmt::Display) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(msg.to_string())]))
}

fn tool_success(value: &impl serde::Serialize) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(
        rmcp::serde_json::to_string(value).unwrap_or_default(),
    )]))
}

fn detail_level(full: Option<bool>) -> ContentDetail {
    match full.unwrap_or(false) {
        true => ContentDetail::Full,
        false => ContentDetail::Summary,
    }
}

fn clamp_limit(limit: Option<i64>) -> Option<i64> {
    Some(limit.unwrap_or(crate::schema::DEFAULT_PAGE_SIZE).clamp(1, crate::schema::MAX_PAGE_SIZE))
}

fn validate_tags(tags: Option<&str>) -> Result<&str, Result<CallToolResult, ErrorData>> {
    let tags = tags.unwrap_or("[]");
    match rmcp::serde_json::from_str::<Vec<rmcp::serde_json::Value>>(tags) {
        Ok(arr) => {
            if arr.iter().all(|v| v.is_string()) {
                Ok(tags)
            } else {
                Err(tool_error("tags must be a JSON array of strings"))
            }
        }
        Err(_) => Err(tool_error("tags must be a valid JSON array")),
    }
}

#[tool_router]
impl MementoServer {
    #[tool(
        description = "Store or update a memory. Use version 1 for new memories. For updates, set version to current version + 1. A version mismatch returns a conflict with the current state so you can resolve and retry."
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
        let tags = match validate_tags(input.tags.as_deref()) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let content = compact_content(&input.content);
        match memory::write(
            &self.pool,
            &input.key,
            &content,
            memory_type,
            input.project.as_deref(),
            tags,
            input.version,
        )
        .await
        {
            Ok(result) => tool_success(&result),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Get the version history of a memory. Returns previous versions in reverse chronological order.")]
    async fn get_history(
        &self,
        Parameters(input): Parameters<GetHistoryInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match memory::get_history(&self.pool, &input.key, clamp_limit(input.limit)).await {
            Ok(versions) => tool_success(&versions),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Write multiple memories in a single call. Returns per-item results — each item independently succeeds or returns a version conflict. Retry conflicting items after incrementing their version.")]
    async fn write_memories(
        &self,
        Parameters(input): Parameters<WriteMemoriesBatchInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut results = Vec::with_capacity(input.memories.len());
        for item in &input.memories {
            let Some(memory_type) = MemoryType::parse(&item.memory_type) else {
                return tool_error(format!(
                    "invalid memory_type '{}' on key '{}': must be user, feedback, project, or reference",
                    item.memory_type, item.key
                ));
            };
            let tags = match validate_tags(item.tags.as_deref()) {
                Ok(t) => t,
                Err(e) => return e,
            };
            let content = compact_content(&item.content);
            match memory::write(
                &self.pool,
                &item.key,
                &content,
                memory_type,
                item.project.as_deref(),
                tags,
                item.version,
            )
            .await
            {
                Ok(result) => results.push(result),
                Err(e) => return tool_error(format!("failed on key '{}': {e}", item.key)),
            }
        }
        tool_success(&results)
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
        description = "Search memories by text query with optional type and project filters. Returns summaries by default — use read_memory for full content. Set full=true to get complete content."
    )]
    async fn search_memories(
        &self,
        Parameters(input): Parameters<SearchMemoriesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = memory::ListParams {
            memory_type: input.memory_type.as_deref().and_then(MemoryType::parse),
            project: input.project.as_deref(),
            tag: input.tag.as_deref(),
            cursor: input.cursor.as_deref(),
            limit: clamp_limit(input.limit),
            detail: detail_level(input.full),
        };
        match memory::search(&self.pool, &input.query, &params).await {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }

    #[tool(
        description = "List memories with optional type and project filters. Returns summaries by default — use read_memory for full content. Set full=true to get complete content."
    )]
    async fn list_memories(
        &self,
        Parameters(input): Parameters<ListMemoriesInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let params = memory::ListParams {
            memory_type: input.memory_type.as_deref().and_then(MemoryType::parse),
            project: input.project.as_deref(),
            tag: input.tag.as_deref(),
            cursor: input.cursor.as_deref(),
            limit: clamp_limit(input.limit),
            detail: detail_level(input.full),
        };
        match memory::list(&self.pool, &params).await {
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
        let content = compact_content(&input.content);
        match artifact::write(
            &self.pool,
            &input.key,
            &content,
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

    #[tool(description = "Write multiple artifacts in a single call. Each entry follows the same schema as write_artifact.")]
    async fn write_artifacts(
        &self,
        Parameters(input): Parameters<WriteArtifactsBatchInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut results = Vec::with_capacity(input.artifacts.len());
        for item in &input.artifacts {
            let expires_at = match item.expires_at.as_deref() {
                Some(s) => match time::OffsetDateTime::parse(
                    s,
                    &time::format_description::well_known::Rfc3339,
                ) {
                    Ok(dt) => Some(dt),
                    Err(e) => {
                        return tool_error(format!(
                            "invalid expires_at on key '{}': {e}",
                            item.key
                        ))
                    }
                },
                None => None,
            };
            let content = compact_content(&item.content);
            match artifact::write(
                &self.pool,
                &item.key,
                &content,
                &item.artifact_type,
                item.project.as_deref(),
                item.source_agent.as_deref(),
                expires_at,
            )
            .await
            {
                Ok(row) => results.push(row),
                Err(e) => return tool_error(format!("failed on key '{}': {e}", item.key)),
            }
        }
        tool_success(&results)
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
        description = "List artifacts with optional type and project filters. Expired artifacts are excluded. Returns summaries by default — use read_artifact for full content. Set full=true to get complete content."
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
            clamp_limit(input.limit),
            detail_level(input.full),
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
            clamp_limit(input.limit),
        )
        .await
        {
            Ok(page) => tool_success(&page),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Create a directed relationship between two entities (memories or artifacts). Supports relationship types like relates_to, depends_on, supersedes, derived_from.")]
    async fn link(
        &self,
        Parameters(input): Parameters<LinkInput>,
    ) -> Result<CallToolResult, ErrorData> {
        if !is_valid_entity_type(&input.source_type) {
            return tool_error(format!("invalid source_type '{}': must be memory or artifact", input.source_type));
        }
        if !is_valid_entity_type(&input.target_type) {
            return tool_error(format!("invalid target_type '{}': must be memory or artifact", input.target_type));
        }
        match relationship::link(
            &self.pool,
            &input.source_type,
            &input.source_key,
            &input.target_type,
            &input.target_key,
            &input.relation,
        )
        .await
        {
            Ok(row) => tool_success(&row),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Remove a relationship between two entities. Omit relation to remove all relationships between the pair.")]
    async fn unlink(
        &self,
        Parameters(input): Parameters<UnlinkInput>,
    ) -> Result<CallToolResult, ErrorData> {
        match relationship::unlink(
            &self.pool,
            &input.source_type,
            &input.source_key,
            &input.target_type,
            &input.target_key,
            input.relation.as_deref(),
        )
        .await
        {
            Ok(count) => tool_success(&rmcp::serde_json::json!({ "removed": count })),
            Err(e) => tool_error(e),
        }
    }

    #[tool(description = "Get all relationships for an entity, both outgoing (this entity as source) and incoming (this entity as target). Optionally filter by relationship type.")]
    async fn get_related(
        &self,
        Parameters(input): Parameters<GetRelatedInput>,
    ) -> Result<CallToolResult, ErrorData> {
        if !is_valid_entity_type(&input.entity_type) {
            return tool_error(format!("invalid entity_type '{}': must be memory or artifact", input.entity_type));
        }
        match relationship::get_related(
            &self.pool,
            &input.entity_type,
            &input.key,
            input.relation.as_deref(),
        )
        .await
        {
            Ok(related) => tool_success(&related),
            Err(e) => tool_error(e),
        }
    }
}

fn is_valid_entity_type(t: &str) -> bool {
    matches!(t, "memory" | "artifact")
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
