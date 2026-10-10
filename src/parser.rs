use crate::model::*;
use chrono::{DateTime, Utc};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

pub fn sessions_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/jackson".to_string());
    PathBuf::from(home)
        .join(".pi")
        .join("agent")
        .join("sessions")
}

/// Discover all session JSONL files, sorted by start_time descending.
pub fn discover_sessions(since_days: Option<f64>) -> Vec<SessionFile> {
    let base = sessions_dir();
    let mut files = Vec::new();

    let dirs = match fs::read_dir(&base) {
        Ok(d) => d,
        Err(_) => return files,
    };

    for dir_entry in dirs.flatten() {
        let dir_path = dir_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let dir_name = dir_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let entries = match fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for file_entry in entries.flatten() {
            let path = file_entry.path();
            if path.extension().map(|e| e.to_string_lossy() != "jsonl").unwrap_or(true) {
                continue;
            }

            // Filename format: 2026-08-08T15-50-38-606Z_UUID.jsonl
            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if let Some(ts) = parse_timestamp_from_filename(&fname) {
                // Extract UUID from filename
                let id = fname
                    .strip_suffix(".jsonl")
                    .unwrap_or(&fname)
                    .splitn(2, '_')
                    .nth(1)
                    .unwrap_or("unknown")
                    .to_string();

                if let Some(days) = since_days {
                    let age = Utc::now() - ts;
                    if age.num_seconds() < 0 || (age.num_days() as f64) > days {
                        continue;
                    }
                }

                files.push(SessionFile {
                    path,
                    id,
                    dir: dir_name.clone(),
                    start_time: ts,
                });
            }
        }
    }

    files.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    files
}

fn parse_timestamp_from_filename(fname: &str) -> Option<DateTime<Utc>> {
    // Filename: 2026-10-10T02-59-54-172Z_UUID.jsonl
    // We want:  2026-10-10T02:59:54.172Z
    let ts_str = fname.split('_').next()?;
    let (date_part, time_part) = ts_str.split_once('T')?;
    // time_part: "02-59-54-172Z"
    let time_part = time_part.trim_end_matches('Z');
    let parts: Vec<&str> = time_part.splitn(4, '-').collect();
    if parts.len() != 4 { return None; }
    let rfc = format!("{}T{}:{}:{}.{}Z", date_part, parts[0], parts[1], parts[2], parts[3]);
    DateTime::parse_from_rfc3339(&rfc)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

/// Parse a session file and extract messages matching filter
pub fn parse_session(
    file: &SessionFile,
    filter: &Filter,
    pattern: Option<&str>,
) -> Vec<MessageInfo> {
    let f = match fs::File::open(&file.path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let reader = BufReader::new(f);
    let mut results = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let entry: Entry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match entry {
            Entry::Message { id, timestamp, message } => {
                if let Some(info) = extract_message(&file.id, &id, &timestamp, &message, filter, pattern) {
                    results.push(info);
                }
            }
            Entry::Compaction { id, timestamp, summary } => {
                // Treat compaction as an assistant message for search purposes
                if filter.match_roles.contains(&Role::Compaction) {
                    if matches_pattern(&summary, pattern) {
                        results.push(MessageInfo {
                            session_id: file.id.clone(),
                            entry_id: id,
                            timestamp,
                            role: "compaction".to_string(),
                            text: summary.clone(),
                            tool_name: None,
                            tool_args: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    results
}

#[derive(PartialEq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    Compaction,
}

pub struct Filter {
    pub match_roles: Vec<Role>,
    pub context_lines: usize,
}

fn extract_message(
    session_id: &str,
    entry_id: &str,
    timestamp: &str,
    body: &MessageBody,
    filter: &Filter,
    pattern: Option<&str>,
) -> Option<MessageInfo> {
    let role = match body.role.as_str() {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "toolResult" => Role::Tool,
        _ => return None,
    };

    if !filter.match_roles.iter().any(|r| std::mem::discriminant(r) == std::mem::discriminant(&role)) {
        return None;
    }

    let role_name = match body.role.as_str() {
        "user" => "user",
        "assistant" => "assistant",
        "toolResult" => "tool",
        _ => return None,
    };

    match role {
        Role::User => {
            let text = body.content.iter().filter_map(|b| {
                if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
            }).collect::<Vec<_>>().join("\n");
            if text.is_empty() { return None; }
            if matches_pattern(&text, pattern) {
                Some(MessageInfo {
                    session_id: session_id.to_string(),
                    entry_id: entry_id.to_string(),
                    timestamp: timestamp.to_string(),
                    role: role_name.to_string(),
                    text,
                    tool_name: None,
                    tool_args: None,
                })
            } else {
                None
            }
        }
        Role::Assistant => {
            let text = body.content.iter().filter_map(|b| {
                if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
            }).collect::<Vec<_>>().join("\n");
            if text.is_empty() { return None; }
            if matches_pattern(&text, pattern) {
                Some(MessageInfo {
                    session_id: session_id.to_string(),
                    entry_id: entry_id.to_string(),
                    timestamp: timestamp.to_string(),
                    role: role_name.to_string(),
                    text,
                    tool_name: None,
                    tool_args: None,
                })
            } else {
                None
            }
        }
        Role::Tool => {
            let tool_name = body.tool_name.clone().unwrap_or_default();
            // For tool results, match against tool name or content
            let text = body.content.iter().filter_map(|b| {
                if let ContentBlock::Text { text } = b { Some(text.as_str()) } else { None }
            }).collect::<Vec<_>>().join("\n");

            let matches_name = pattern.map(|p| tool_name.to_lowercase().contains(&p.to_lowercase())).unwrap_or(true);
            let matches_text = matches_pattern(&text, pattern);

            if matches_name || matches_text {
                // Truncate tool result content in output
                let display_text = if text.len() > 200 {
                    format!("{}... [truncated]", &text[..200])
                } else {
                    text
                };
                Some(MessageInfo {
                    session_id: session_id.to_string(),
                    entry_id: entry_id.to_string(),
                    timestamp: timestamp.to_string(),
                    role: role_name.to_string(),
                    text: display_text,
                    tool_name: Some(tool_name),
                    tool_args: None,
                })
            } else {
                None
            }
        }
        Role::Compaction => None,
    }
}

fn matches_pattern(text: &str, pattern: Option<&str>) -> bool {
    match pattern {
        None => true,
        Some(p) => text.to_lowercase().contains(&p.to_lowercase()),
    }
}

/// Extract tool calls from assistant messages
pub fn extract_tool_calls(file: &SessionFile, name_filter: Option<&str>, file_pattern: Option<&str>) -> Vec<ToolCallInfo> {
    let f = match fs::File::open(&file.path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let reader = BufReader::new(f);
    let mut results = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let entry: Entry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Entry::Message { id, timestamp, message } = entry {
            if message.role != "assistant" { continue; }
            for block in &message.content {
                if let ContentBlock::ToolCall { id: call_id, name, arguments } = block {
                    // Apply name filter
                    if let Some(nf) = name_filter {
                        if !name.to_lowercase().contains(&nf.to_lowercase()) {
                            continue;
                        }
                    }
                    // Apply file pattern filter
                    if let Some(fp) = file_pattern {
                        let args_str = arguments.as_ref().map(|a| a.to_string()).unwrap_or_default();
                        if !glob_match_simple(fp, &args_str) {
                            continue;
                        }
                    }
                    results.push(ToolCallInfo {
                        session_id: file.id.clone(),
                        entry_id: id.clone(),
                        call_id: call_id.clone(),
                        timestamp: timestamp.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    });
                }
            }
        }
    }

    results
}

pub struct ToolCallInfo {
    pub session_id: String,
    pub entry_id: String,
    pub call_id: String,
    pub timestamp: String,
    pub name: String,
    pub arguments: Option<serde_json::Value>,
}

fn glob_match_simple(pattern: &str, text: &str) -> bool {
    // Simple glob: just check if the pattern (minus glob chars) appears in text
    let clean_pattern = pattern.replace('*', "").replace('?', "");
    text.contains(&clean_pattern)
}
