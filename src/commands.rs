use crate::model::*;
use crate::parser::*;


pub fn cmd_ls(since: f64, limit: usize, json: bool) {
    let sessions = discover_sessions(Some(since));
    let sessions: Vec<_> = sessions.into_iter().take(limit).collect();

    if json {
        let items: Vec<_> = sessions.iter().map(|s| serde_json::json!({
            "id": s.id,
            "dir": s.dir,
            "start_time": s.start_time.to_rfc3339(),
            "mtime": s.mtime.to_rfc3339(),
            "path": s.path.to_string_lossy(),
        })).collect();
        println!("{}", serde_json::to_string_pretty(&items).unwrap());
    } else {
        if sessions.is_empty() {
            println!("No sessions active in the last {:.0} days.", since);
            return;
        }
        // Ordered by last activity. STARTED is shown too so a session that was resumed
        // days after it began is visible as such instead of looking stale.
        println!("{:<4} {:<16} {:<16} {:<36} {}", "#", "LAST ACTIVE", "STARTED", "SESSION ID", "DIR");
        println!("{}", "─".repeat(110));
        for (i, s) in sessions.iter().enumerate() {
            let active = local_dt(&s.mtime);
            let started = local_dt(&s.start_time);
            println!("{:<4} {:<16} {:<16} {:<36} {}", i + 1, active, started, s.id, s.dir);
        }
        println!();
        println!("{}", tz_note());
    }
}

pub fn cmd_grep(
    pattern: Option<&str>,
    user: bool,
    assistant: bool,
    tool: bool,
    compaction: bool,
    since: f64,
    session_id: Option<&str>,
    context: usize,
    limit: usize,
    json: bool,
) {
    let mut roles = Vec::new();
    // If no role flags, match all (compaction is opt-in: it would otherwise
    // re-surface the same recent text the live conversation already has).
    if !user && !assistant && !tool && !compaction {
        roles.push(Role::User);
        roles.push(Role::Assistant);
        roles.push(Role::Tool);
    } else {
        if user { roles.push(Role::User); }
        if assistant { roles.push(Role::Assistant); }
        if tool { roles.push(Role::Tool); }
        if compaction { roles.push(Role::Compaction); }
    }

    // If no pattern, show recent turns of the selected roles
    let effective_pattern = pattern;

    let filter = Filter {
        match_roles: roles,
        context_lines: context,
    };

    let sessions = if let Some(sid) = session_id {
        // Find specific session
        discover_sessions(None)
            .into_iter()
            .filter(|s| s.id == sid || s.id.starts_with(sid))
            .collect()
    } else {
        discover_sessions(Some(since))
    };

    let mut all_results: Vec<MessageInfo> = Vec::new();
    for session in &sessions {
        let results = parse_session(session, &filter, effective_pattern);
        all_results.extend(results);
        if all_results.len() >= limit {
            break;
        }
    }

    let results: Vec<_> = all_results.into_iter().take(limit).collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&results.iter().map(|r| serde_json::json!({
            "session_id": r.session_id,
            "entry_id": r.entry_id,
            "timestamp": r.timestamp,
            "role": r.role,
            "text": r.text,
            "tool_name": r.tool_name,
        })).collect::<Vec<_>>()).unwrap());
    } else {
        if results.is_empty() {
            println!("No matches found.");
            return;
        }
        let total = results.len();
        println!("{} match(es):\n", total);
        println!("{}", tz_note());
        println!();
        for (i, r) in results.iter().enumerate() {
            let ts = local_ts(&r.timestamp, false);
            let sid_short = &r.session_id[..8.min(r.session_id.len())];
            let eid_short = &r.entry_id[..8.min(r.entry_id.len())];
            println!("[{}/{}] {} {} {} {}", i + 1, total, ts, sid_short, eid_short, r.role);
            if let Some(ref tn) = r.tool_name {
                println!("  tool: {}", tn);
            }
            // Show text (truncated for readability)
            for line in r.text.lines().take(if context == 0 { 3 } else { context.max(3) }) {
                println!("  | {}", line);
            }
            println!();
        }
    }
}

pub fn cmd_show(
    session_id: &str,
    user: bool,
    assistant: bool,
    tool: bool,
    around: Option<&str>,
    json: bool,
) {
    let sessions = discover_sessions(None);
    let session = sessions.iter().find(|s| s.id == session_id || s.id.starts_with(session_id));
    let session = match session {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {}", session_id);
            eprintln!("Try: piq ls --since 90");
            return;
        }
    };

    let mut roles = Vec::new();
    if !user && !assistant && !tool {
        roles.push(Role::User);
        roles.push(Role::Assistant);
        roles.push(Role::Compaction);
    } else {
        if user { roles.push(Role::User); }
        if assistant { roles.push(Role::Assistant); }
        if tool { roles.push(Role::Tool); }
    }

    let filter = Filter {
        match_roles: roles,
        context_lines: 0,
    };

    let results = parse_session(session, &filter, None);

    // If --around, filter to messages near the given entry_id
    let results = if let Some(eid) = around {
        let idx = results.iter().position(|r| r.entry_id == eid || r.entry_id.starts_with(eid));
        match idx {
            Some(i) => {
                let start = i.saturating_sub(5);
                let end = (i + 6).min(results.len());
                results[start..end].to_vec()
            }
            None => {
                eprintln!("Entry ID not found: {}", eid);
                return;
            }
        }
    } else {
        results
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&results.iter().map(|r| serde_json::json!({
            "entry_id": r.entry_id,
            "timestamp": r.timestamp,
            "role": r.role,
            "text": r.text,
            "tool_name": r.tool_name,
        })).collect::<Vec<_>>()).unwrap());
    } else {
        if results.is_empty() {
            println!("No messages found.");
            return;
        }
        println!("Session: {}", session.id);
        println!("File: {}", session.path.to_string_lossy());
        println!("{} messages\n", results.len());
        println!("{}", tz_note());
        println!();
        for r in &results {
            let ts = local_ts(&r.timestamp, true);
            println!("── {} [{}] {} ──", ts, &r.entry_id[..8.min(r.entry_id.len())], r.role);
            for line in r.text.lines() {
                println!("  {}", line);
            }
            println!();
        }
    }
}

pub fn cmd_tools(
    since: f64,
    name: Option<&str>,
    file_pat: Option<&str>,
    limit: usize,
    json: bool,
) {
    let sessions = discover_sessions(Some(since));
    let mut all_calls: Vec<ToolCallInfo> = Vec::new();

    for session in &sessions {
        let calls = extract_tool_calls(session, name, file_pat);
        all_calls.extend(calls);
        if all_calls.len() >= limit {
            break;
        }
    }

    let results: Vec<_> = all_calls.into_iter().take(limit).collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&results.iter().map(|r| serde_json::json!({
            "session_id": r.session_id,
            "entry_id": r.entry_id,
            "timestamp": r.timestamp,
            "name": r.name,
            "arguments": r.arguments,
        })).collect::<Vec<_>>()).unwrap());
    } else {
        if results.is_empty() {
            println!("No tool calls found.");
            return;
        }
        println!("{} tool call(s):\n", results.len());
        println!("{}", tz_note());
        println!();
        for (i, r) in results.iter().enumerate() {
            let ts = local_ts(&r.timestamp, false);
            let sid_short = &r.session_id[..8.min(r.session_id.len())];
            let args_str = summarize_args(&r.arguments);
            println!("[{}/{}] {} {} {} {}", i + 1, results.len(), ts, sid_short, r.name, args_str);
        }
    }
}

pub fn cmd_compacts(
    since: f64,
    session_id: Option<&str>,
    show: bool,
    limit: usize,
    json: bool,
) {
    let sessions = if let Some(sid) = session_id {
        discover_sessions(None)
            .into_iter()
            .filter(|s| s.id == sid || s.id.starts_with(sid))
            .collect()
    } else {
        discover_sessions(Some(since))
    };

    let mut all: Vec<CompactionInfo> = Vec::new();
    for s in &sessions {
        all.extend(extract_compactions(s));
        if all.len() >= limit {
            break;
        }
    }
    let results: Vec<_> = all.into_iter().take(limit).collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&results.iter().map(|r| serde_json::json!({
            "session_id": r.session_id,
            "entry_id": r.entry_id,
            "timestamp": r.timestamp,
            "tokens_before": r.tokens_before,
            "first_kept_entry_id": r.first_kept_entry_id,
            "summary": r.summary,
        })).collect::<Vec<_>>()).unwrap());
        return;
    }

    if results.is_empty() {
        println!("No compaction events found.");
        return;
    }
    println!("{} compaction(s):\n", results.len());
    println!("{}", tz_note());
    println!();
    for (i, r) in results.iter().enumerate() {
        let ts = local_ts(&r.timestamp, false);
        let sid_short = &r.session_id[..8.min(r.session_id.len())];
        let eid_short = &r.entry_id[..8.min(r.entry_id.len())];
        println!("[{}/{}] {} {} {}  tokens_before={}  summary={} chars",
            i + 1, results.len(), ts, sid_short, eid_short, r.tokens_before, r.summary.chars().count());
        if show {
            for line in r.summary.lines() {
                println!("  | {}", line);
            }
        }
        println!();
    }
}

pub fn cmd_last(
    user: bool,
    assistant: bool,
    tool: bool,
    limit: usize,
    json: bool,
) {
    // PI_SESSION_ID is exported only into the environment of a live pi session, so this
    // skip fires when an agent runs piq and is inert for a human in a plain shell. The
    // point is to make `last` mean the same thing for both: "the last conversation other
    // than the one I am in", instead of depending on which of us invoked it.
    let self_id = std::env::var("PI_SESSION_ID").ok();
    let is_self = |s: &SessionFile| match &self_id {
        Some(me) => s.id == *me || me.starts_with(&s.id) || s.id.starts_with(me),
        None => false,
    };

    let sessions = discover_sessions(Some(30.0)); // look back 30 days for last session
    let session = match sessions.iter().find(|s| !is_self(s)) {
        Some(s) => s,
        None => {
            if self_id.is_some() {
                eprintln!(
                    "Only session in range is the current one ({}).",
                    self_id.unwrap()
                );
            } else {
                eprintln!("No sessions found.");
            }
            return;
        }
    };

    let mut roles = Vec::new();
    if !user && !assistant && !tool {
        roles.push(Role::User);
        roles.push(Role::Assistant);
    } else {
        if user { roles.push(Role::User); }
        if assistant { roles.push(Role::Assistant); }
        if tool { roles.push(Role::Tool); }
    }

    let filter = Filter { match_roles: roles, context_lines: 0 };
    let results = parse_session(session, &filter, None);
    let total = results.len();

    // Take last N
    let start = results.len().saturating_sub(limit);
    let results = &results[start..];

    if json {
        println!("{}", serde_json::to_string_pretty(&results.iter().map(|r| serde_json::json!({
            "session_id": r.session_id,
            "entry_id": r.entry_id,
            "timestamp": r.timestamp,
            "role": r.role,
            "text": r.text,
        })).collect::<Vec<_>>()).unwrap());
    } else {
        println!(
            "Session: {}  last active {}  (started {})",
            session.id,
            local_dt(&session.mtime),
            local_dt(&session.start_time)
        );
        if let Some(me) = &self_id {
            println!("(skipped current session {})", me);
        }
        println!("{} messages shown of {}", results.len(), total);
        println!("{}", tz_note());
        println!();
        for r in results {
            let ts = local_ts(&r.timestamp, true);
            println!("── {} [{}] {} ──", ts, &r.entry_id[..8.min(r.entry_id.len())], r.role);
            for line in r.text.lines() {
                println!("  {}", line);
            }
            println!();
        }
    }
}

fn summarize_args(args: &Option<serde_json::Value>) -> String {
    match args {
        None => String::new(),
        Some(v) => {
            if let Some(obj) = v.as_object() {
                // For bash: show command. For edit/write: show path. For others: first value.
                if let Some(cmd) = obj.get("command").and_then(|c| c.as_str()) {
                    let first_line = cmd.lines().next().unwrap_or("");
                    format!("\"{}\"", truncate(first_line, 60))
                } else if let Some(path) = obj.get("path").and_then(|c| c.as_str()) {
                    format!("({})", path)
                } else {
                    let s = v.to_string();
                    format!("{}", truncate(&s, 50))
                }
            } else {
                truncate(&v.to_string(), 50).to_string()
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    // Cut on a character boundary, not a byte offset — `&s[..max]` panics mid-character.
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}
