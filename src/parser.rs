use crate::model::*;
use chrono::{DateTime, Local, Utc};
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

/// Human-facing timestamp: the entry's UTC ISO-8601 rendered in the machine's local zone.
/// `with_seconds` keeps seconds for the dense per-message dumps (show/last); the one-line
/// index views (ls/grep/tools/compacts) do not need them.
pub fn local_ts(ts: &str, with_seconds: bool) -> String {
    let width = if with_seconds { 19 } else { 16 };
    match DateTime::parse_from_rfc3339(ts) {
        Ok(t) => t
            .with_timezone(&Local)
            .format(if with_seconds { "%Y-%m-%d %H:%M:%S" } else { "%Y-%m-%d %H:%M" })
            .to_string(),
        // Unparseable timestamp: show what we have rather than nothing.
        Err(_) => ts.get(..width).unwrap_or(ts).to_string(),
    }
}

/// Render a UTC DateTime in the local zone for display.
pub fn local_dt(t: &DateTime<Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

/// Which clock the printed timestamps are on. Printed once per command; `--json` output is
/// deliberately left in UTC because it is meant to be parsed, not read.
pub fn tz_note() -> String {
    // chrono's %Z on a DateTime<Local> yields the numeric offset, not the zone abbreviation,
    // so ask `date` for the same name the user's own shell prints. This also honours TZ if the
    // user has overridden it. Falls back to the bare offset if the spawn fails.
    let label = std::process::Command::new("date")
        .arg("+%Z")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_alphabetic()))
        .unwrap_or_else(|| "local".to_string());

    // Offset comes from chrono, which is authoritative for the rendered timestamps themselves.
    let mins = Local::now().offset().local_minus_utc() / 60;
    let (sign, abs) = if mins < 0 { ('-', -mins) } else { ('+', mins) };
    let hours = abs / 60;
    let offset = if abs % 60 == 0 {
        format!("UTC{}{}", sign, hours)
    } else {
        format!("UTC{}{:02}:{:02}", sign, hours, abs % 60)
    };

    format!(
        "# timestamps local: {} ({})  |  --json stays UTC",
        label, offset
    )
}

/// Discover all session JSONL files, sorted by mtime (last activity) descending.
/// Ordering and the `since` window both use mtime rather than the filename timestamp:
/// a resumed session keeps its original filename but is the most recent conversation.
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

                // mtime == when the last entry was appended. Falls back to start_time
                // if the filesystem refuses to report it.
                let mtime = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(DateTime::<Utc>::from)
                    .unwrap_or(ts);

                if let Some(days) = since_days {
                    let age = Utc::now() - mtime;
                    // Allow a little clock skew before discarding a file as "from the
                    // future"; dropping the newest session is the worst possible failure.
                    if age.num_seconds() < -3600 {
                        continue;
                    }
                    // num_days() truncates, so the old `age.num_days() > days` test let
                    // -s 3 reach almost 4 days back. Compare real fractional age.
                    if (age.num_seconds() as f64 / 86400.0) > days {
                        continue;
                    }
                }

                files.push(SessionFile {
                    path,
                    id,
                    dir: dir_name.clone(),
                    start_time: ts,
                    mtime,
                });
            }
        }
    }

    files.sort_by(|a, b| b.mtime.cmp(&a.mtime));
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
            Entry::Compaction { id, timestamp, summary, .. } => {
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

/// Extract compaction events (session summaries) across a session
pub fn extract_compactions(file: &SessionFile) -> Vec<CompactionInfo> {
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
        if let Entry::Compaction { id, timestamp, summary, tokens_before, first_kept_entry_id } = entry {
            results.push(CompactionInfo {
                session_id: file.id.clone(),
                entry_id: id,
                timestamp,
                tokens_before,
                first_kept_entry_id,
                summary,
            });
        }
    }
    results
}

pub struct CompactionInfo {
    pub session_id: String,
    pub entry_id: String,
    pub timestamp: String,
    pub tokens_before: u64,
    pub first_kept_entry_id: String,
    pub summary: String,
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
