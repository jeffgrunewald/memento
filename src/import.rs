use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::memory;
use crate::schema::MemoryType;

// -- Public types --

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Source {
    /// Claude Code (~/.claude/projects/*/memory/)
    Claude,
    /// Cursor (.cursor/rules/, .cursorrules)
    Cursor,
    /// Windsurf (~/.codeium/windsurf/memories/, .windsurf/rules/, .windsurfrules)
    Windsurf,
    /// Roo Code (~/.roo/rules/, .roo/rules/, .roorules, memory-bank/)
    RooCode,
    /// All supported frameworks
    All,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub imported: Vec<ImportedItem>,
    pub skipped: Vec<SkippedItem>,
    pub errors: Vec<ErrorItem>,
}

#[derive(Debug, Serialize)]
pub struct ImportedItem {
    pub key: String,
    pub source: String,
    pub memory_type: String,
    pub project: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SkippedItem {
    pub key: String,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorItem {
    pub source: String,
    pub error: String,
}

// -- Discovery --

struct DiscoveredFile {
    path: PathBuf,
    project: Option<String>,
    source_framework: &'static str,
}

fn discover(source: Source, workspace: Option<&Path>) -> Vec<DiscoveredFile> {
    let home = std::env::var("HOME").unwrap_or_default();
    let home = Path::new(&home);

    let mut files = vec![];
    match source {
        Source::Claude => discover_claude(home, &mut files),
        Source::Cursor => discover_cursor(home, workspace, &mut files),
        Source::Windsurf => discover_windsurf(home, workspace, &mut files),
        Source::RooCode => discover_roo_code(home, workspace, &mut files),
        Source::All => {
            discover_claude(home, &mut files);
            discover_cursor(home, workspace, &mut files);
            discover_windsurf(home, workspace, &mut files);
            discover_roo_code(home, workspace, &mut files);
        }
    }
    files
}

fn discover_claude(home: &Path, files: &mut Vec<DiscoveredFile>) {
    let projects_dir = home.join(".claude/projects");
    let Ok(entries) = std::fs::read_dir(&projects_dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let memory_dir = entry.path().join("memory");
        if !memory_dir.is_dir() {
            continue;
        }
        let project = claude_project_from_path(&memory_dir);
        scan_dir(&memory_dir, project.as_deref(), "claude", files, |p| {
            let ext = p.extension().and_then(|e| e.to_str());
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == Some("md") && name != "MEMORY.md"
        });
    }
}

fn discover_cursor(home: &Path, workspace: Option<&Path>, files: &mut Vec<DiscoveredFile>) {
    // Project-level: .cursor/rules/*.mdc
    if let Some(ws) = workspace {
        let rules_dir = ws.join(".cursor/rules");
        scan_dir(&rules_dir, None, "cursor", files, |p| {
            p.extension().and_then(|e| e.to_str()) == Some("mdc")
        });

        // Legacy: .cursorrules
        collect_file(&ws.join(".cursorrules"), None, "cursor", files);
    }

    // Global: Cursor stores global rules in its DB, not filesystem.
    // We can't import those. But check for AGENTS.md at workspace root.
    if let Some(ws) = workspace {
        collect_file(&ws.join("AGENTS.md"), None, "cursor", files);
    }

    // Also scan home for any .cursor project dirs that have rules
    let cursor_dir = home.join(".cursor");
    if cursor_dir.is_dir() {
        scan_dir_recursive(&cursor_dir.join("rules"), None, "cursor", files, |p| {
            let ext = p.extension().and_then(|e| e.to_str());
            ext == Some("mdc") || ext == Some("md")
        });
    }
}

fn discover_windsurf(home: &Path, workspace: Option<&Path>, files: &mut Vec<DiscoveredFile>) {
    // Global memories
    let memories_dir = home.join(".codeium/windsurf/memories");
    scan_dir(&memories_dir, None, "windsurf", files, |p| {
        p.extension().and_then(|e| e.to_str()) == Some("md")
    });

    // Project-level rules
    if let Some(ws) = workspace {
        let rules_dir = ws.join(".windsurf/rules");
        scan_dir(&rules_dir, None, "windsurf", files, |p| {
            p.extension().and_then(|e| e.to_str()) == Some("md")
        });

        // Legacy: .windsurfrules
        collect_file(&ws.join(".windsurfrules"), None, "windsurf", files);
    }
}

fn discover_roo_code(home: &Path, workspace: Option<&Path>, files: &mut Vec<DiscoveredFile>) {
    // Global rules
    let global_rules = home.join(".roo/rules");
    scan_dir_recursive(&global_rules, None, "roo-code", files, |p| {
        let ext = p.extension().and_then(|e| e.to_str());
        ext == Some("md") || ext == Some("txt")
    });

    if let Some(ws) = workspace {
        // Project rules: .roo/rules/
        let project_rules = ws.join(".roo/rules");
        scan_dir_recursive(&project_rules, None, "roo-code", files, |p| {
            let ext = p.extension().and_then(|e| e.to_str());
            ext == Some("md") || ext == Some("txt")
        });

        // Memory bank: memory-bank/
        let memory_bank = ws.join("memory-bank");
        scan_dir(&memory_bank, None, "roo-code", files, |p| {
            p.extension().and_then(|e| e.to_str()) == Some("md")
        });

        // Legacy single files
        collect_file(&ws.join(".roorules"), None, "roo-code", files);
        collect_file(&ws.join(".clinerules"), None, "roo-code", files);
    }
}

// -- Scanning helpers --

fn scan_dir(
    dir: &Path,
    project: Option<&str>,
    framework: &'static str,
    files: &mut Vec<DiscoveredFile>,
    filter: impl Fn(&Path) -> bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && filter(&path) {
            files.push(DiscoveredFile {
                path,
                project: project.map(String::from),
                source_framework: framework,
            });
        }
    }
}

fn scan_dir_recursive(
    dir: &Path,
    project: Option<&str>,
    framework: &'static str,
    files: &mut Vec<DiscoveredFile>,
    filter: impl Fn(&Path) -> bool + Copy,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            scan_dir_recursive(&path, project, framework, files, filter);
        } else if path.is_file() && filter(&path) {
            files.push(DiscoveredFile {
                path,
                project: project.map(String::from),
                source_framework: framework,
            });
        }
    }
}

fn collect_file(
    path: &Path,
    project: Option<&str>,
    framework: &'static str,
    files: &mut Vec<DiscoveredFile>,
) {
    if path.is_file() {
        files.push(DiscoveredFile {
            path: path.to_path_buf(),
            project: project.map(String::from),
            source_framework: framework,
        });
    }
}

// -- Import engine --

pub async fn import_files(pool: &SqlitePool, source: Source, workspace: Option<&Path>) -> Result<ImportReport> {
    let discovered = discover(source, workspace);
    let mut report = ImportReport {
        imported: vec![],
        skipped: vec![],
        errors: vec![],
    };

    for file in &discovered {
        let source_str = file.path.display().to_string();

        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(e) => {
                report.errors.push(ErrorItem {
                    source: source_str,
                    error: e.to_string(),
                });
                continue;
            }
        };

        let parsed = parse_file(&content);

        let key = build_key(&file.path, &parsed, file.project.as_deref(), file.source_framework);
        let memory_type = parsed
            .memory_type
            .as_deref()
            .and_then(MemoryType::parse)
            .unwrap_or(MemoryType::Reference);

        let mut full_content = parsed.body;
        if let Some(desc) = &parsed.description
            && !full_content.starts_with(desc.as_str())
        {
            full_content = format!("{desc}\n\n{full_content}");
        }

        let tags = rmcp::serde_json::to_string(
            &[format!("source:{}", file.source_framework)]
        ).unwrap_or_else(|_| "[]".to_string());

        match memory::write(
            pool,
            &key,
            &full_content,
            memory_type.clone(),
            file.project.as_deref(),
            &tags,
            1,
        )
        .await
        {
            Ok(memory::WriteResult::Ok { .. }) => {
                report.imported.push(ImportedItem {
                    key,
                    source: source_str,
                    memory_type: memory_type.as_str().to_string(),
                    project: file.project.clone(),
                });
            }
            Ok(memory::WriteResult::Conflict { .. }) => {
                report.skipped.push(SkippedItem {
                    key,
                    source: source_str,
                    reason: "key already exists".to_string(),
                });
            }
            Err(e) => {
                report.errors.push(ErrorItem {
                    source: source_str,
                    error: format!("failed to write key '{key}': {e}"),
                });
            }
        }
    }

    Ok(report)
}

// -- Parsing --

struct ParsedFile {
    name: Option<String>,
    description: Option<String>,
    memory_type: Option<String>,
    body: String,
}

fn parse_file(content: &str) -> ParsedFile {
    let trimmed = content.trim();

    if !trimmed.starts_with("---") {
        return ParsedFile {
            name: None,
            description: None,
            memory_type: None,
            body: trimmed.to_string(),
        };
    }

    let after_open = &trimmed[3..];
    let Some(close_idx) = after_open.find("\n---") else {
        return ParsedFile {
            name: None,
            description: None,
            memory_type: None,
            body: trimmed.to_string(),
        };
    };

    let frontmatter = &after_open[..close_idx];
    let body = after_open[close_idx + 4..].trim().to_string();

    let mut name = None;
    let mut description = None;
    let mut memory_type = None;

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(unquote(val.trim()));
        } else if let Some(val) = line.strip_prefix("description:") {
            description = Some(unquote(val.trim()));
        } else if let Some(val) = line.strip_prefix("type:") {
            memory_type = Some(val.trim().to_string());
        }
    }

    ParsedFile {
        name,
        description,
        memory_type,
        body,
    }
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').trim_matches('\'').to_string()
}

// -- Key building --

fn build_key(path: &Path, parsed: &ParsedFile, project: Option<&str>, framework: &str) -> String {
    let base = match &parsed.name {
        Some(name) => slugify(name),
        None => {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            stem.to_string()
        }
    };

    match project {
        Some(p) => format!("{p}/{base}"),
        None => format!("{framework}/{base}"),
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn claude_project_from_path(memory_dir: &Path) -> Option<String> {
    let project_dir = memory_dir.parent()?;
    let dir_name = project_dir.file_name()?.to_str()?;
    let stripped = dir_name.trim_start_matches('-');
    let parts: Vec<&str> = stripped.split("-workspace").collect();
    match parts.last() {
        Some(suffix) if !suffix.is_empty() => Some(suffix.trim_start_matches('-').to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_frontmatter() {
        let content =
            "---\nname: Test Memory\ndescription: A test\ntype: feedback\n---\n\nBody content here.";
        let parsed = parse_file(content);
        assert_eq!(parsed.name.as_deref(), Some("Test Memory"));
        assert_eq!(parsed.description.as_deref(), Some("A test"));
        assert_eq!(parsed.memory_type.as_deref(), Some("feedback"));
        assert_eq!(parsed.body, "Body content here.");
    }

    #[test]
    fn parse_with_quoted_frontmatter() {
        let content =
            "---\ndescription: \"Coding standards for React\"\nglobs: \"src/**/*.tsx\"\nalwaysApply: false\n---\n\nUse functional components.";
        let parsed = parse_file(content);
        assert_eq!(
            parsed.description.as_deref(),
            Some("Coding standards for React")
        );
        assert_eq!(parsed.body, "Use functional components.");
    }

    #[test]
    fn parse_without_frontmatter() {
        let content = "# Just a heading\n\nSome content.";
        let parsed = parse_file(content);
        assert!(parsed.name.is_none());
        assert!(parsed.memory_type.is_none());
        assert_eq!(parsed.body, "# Just a heading\n\nSome content.");
    }

    #[test]
    fn project_extraction() {
        let path = PathBuf::from(
            "/Users/jeff/.claude/projects/-Users-jeff-workspace-multisig-backend/memory",
        );
        assert_eq!(
            claude_project_from_path(&path).as_deref(),
            Some("multisig-backend")
        );

        let root = PathBuf::from("/Users/jeff/.claude/projects/-Users-jeff-workspace/memory");
        assert_eq!(claude_project_from_path(&root), None);
    }

    #[test]
    fn key_slugification() {
        assert_eq!(
            slugify("Don't assume API shapes"),
            "don-t-assume-api-shapes"
        );
        assert_eq!(
            slugify("MCP memory server design"),
            "mcp-memory-server-design"
        );
    }
}
