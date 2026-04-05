use std::fmt::Write;

use crate::schema::{
    ArtifactRow, ArtifactSummaryRow, EventRow, MemoryRow, MemorySummaryRow, PaginatedResponse,
};

const SEPARATOR: &str = "───────────────────────────────────────────────────";

fn write_pagination_footer(buf: &mut String, has_more: bool, next_cursor: &Option<String>) {
    if has_more
        && let Some(cursor) = next_cursor
    {
        let _ = write!(buf, "\n{SEPARATOR}\nMore results available. Cursor: {cursor}");
    }
}

pub fn format_memories(page: &PaginatedResponse<MemoryRow>) -> String {
    if page.items.is_empty() {
        return "No memories found.".to_string();
    }

    let mut buf = String::new();
    for (i, m) in page.items.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(buf);
        }
        let _ = writeln!(buf, "─── {} ───", m.key);
        let _ = write!(buf, "Type: {} | v{}", m.memory_type, m.version);
        if let Some(ref p) = m.project {
            let _ = write!(buf, " | Project: {p}");
        }
        let _ = writeln!(buf);
        if m.tags != "[]" {
            let _ = writeln!(buf, "Tags: {}", m.tags);
        }
        let _ = writeln!(
            buf,
            "Updated: {}",
            m.updated_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        );
        let _ = writeln!(buf);
        let _ = write!(buf, "{}", m.content.trim_end());
        let _ = writeln!(buf);
    }
    write_pagination_footer(&mut buf, page.has_more, &page.next_cursor);
    buf
}

pub fn format_memory_summaries(page: &PaginatedResponse<MemorySummaryRow>) -> String {
    if page.items.is_empty() {
        return "No memories found.".to_string();
    }

    let mut buf = String::new();
    for (i, m) in page.items.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(buf);
        }
        let _ = writeln!(buf, "─── {} ───", m.key);
        let _ = write!(buf, "Type: {} | v{}", m.memory_type, m.version);
        if let Some(ref p) = m.project {
            let _ = write!(buf, " | Project: {p}");
        }
        let _ = writeln!(buf);
        if m.tags != "[]" {
            let _ = writeln!(buf, "Tags: {}", m.tags);
        }
        let _ = writeln!(buf, "Preview: {}", m.content_preview);
    }
    write_pagination_footer(&mut buf, page.has_more, &page.next_cursor);
    buf
}

pub fn format_artifacts(page: &PaginatedResponse<ArtifactRow>) -> String {
    if page.items.is_empty() {
        return "No artifacts found.".to_string();
    }

    let mut buf = String::new();
    for (i, a) in page.items.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(buf);
        }
        let _ = writeln!(buf, "─── {} ───", a.key);
        let _ = write!(buf, "Type: {}", a.artifact_type);
        if let Some(ref p) = a.project {
            let _ = write!(buf, " | Project: {p}");
        }
        if let Some(ref agent) = a.source_agent {
            let _ = write!(buf, " | Agent: {agent}");
        }
        let _ = writeln!(buf);
        let _ = writeln!(
            buf,
            "Created: {}",
            a.created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        );
        if let Some(ref exp) = a.expires_at {
            let _ = writeln!(
                buf,
                "Expires: {}",
                exp.format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default()
            );
        }
        let _ = writeln!(buf);
        let _ = write!(buf, "{}", a.content.trim_end());
        let _ = writeln!(buf);
    }
    write_pagination_footer(&mut buf, page.has_more, &page.next_cursor);
    buf
}

pub fn format_artifact_summaries(page: &PaginatedResponse<ArtifactSummaryRow>) -> String {
    if page.items.is_empty() {
        return "No artifacts found.".to_string();
    }

    let mut buf = String::new();
    for (i, a) in page.items.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(buf);
        }
        let _ = writeln!(buf, "─── {} ───", a.key);
        let _ = write!(buf, "Type: {}", a.artifact_type);
        if let Some(ref p) = a.project {
            let _ = write!(buf, " | Project: {p}");
        }
        if let Some(ref agent) = a.source_agent {
            let _ = write!(buf, " | Agent: {agent}");
        }
        let _ = writeln!(buf);
        let _ = writeln!(buf, "Preview: {}", a.content_preview);
    }
    write_pagination_footer(&mut buf, page.has_more, &page.next_cursor);
    buf
}

pub fn format_events(page: &PaginatedResponse<EventRow>) -> String {
    if page.items.is_empty() {
        return "No events found.".to_string();
    }

    let mut buf = String::new();
    for (i, e) in page.items.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(buf);
        }
        let _ = writeln!(buf, "─── Event #{} ───", e.id);
        let _ = write!(buf, "Type: {} | Agent: {}", e.event_type, e.source_agent);
        if let Some(ref p) = e.project {
            let _ = write!(buf, " | Project: {p}");
        }
        let _ = writeln!(buf);
        let _ = writeln!(
            buf,
            "Created: {}",
            e.created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        );
        // Pretty-print the JSON payload if it's valid JSON
        if let Ok(parsed) = rmcp::serde_json::from_str::<rmcp::serde_json::Value>(&e.payload) {
            if let Ok(pretty) = rmcp::serde_json::to_string_pretty(&parsed) {
                let _ = writeln!(buf);
                let _ = write!(buf, "{pretty}");
                let _ = writeln!(buf);
            }
        } else {
            let _ = writeln!(buf);
            let _ = write!(buf, "{}", e.payload.trim_end());
            let _ = writeln!(buf);
        }
    }
    write_pagination_footer(&mut buf, page.has_more, &page.next_cursor);
    buf
}
