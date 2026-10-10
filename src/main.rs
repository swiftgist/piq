mod model;
mod parser;
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "piq", version, about = "Query pi agent session transcripts")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Search transcripts
    #[command(visible_alias = "g")]
    Grep {
        /// Search pattern (optional; omit to list recent turns)
        pattern: Option<String>,

        /// User turns only
        #[arg(short = 'u', long)]
        user: bool,

        /// Assistant turns only
        #[arg(short = 'a', long)]
        assistant: bool,

        /// Tool calls/results only
        #[arg(short = 't', long)]
        tool: bool,

        /// Days to search back (default: 2)
        #[arg(short = 's', long, default_value_t = 2.0)]
        since: f64,

        /// Restrict to one session (prefix match)
        #[arg(short = 'S', long)]
        session: Option<String>,

        /// Show N extra lines of context per match
        #[arg(short = 'c', long, default_value_t = 0)]
        context: usize,

        /// Max results (default: 30)
        #[arg(short = 'l', long, default_value_t = 30)]
        limit: usize,

        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// Show a session
    #[command(visible_alias = "s")]
    Show {
        /// Session ID (or prefix)
        session_id: String,

        /// User turns only
        #[arg(short = 'u', long)]
        user: bool,

        /// Assistant turns only
        #[arg(short = 'a', long)]
        assistant: bool,

        /// Tool calls only
        #[arg(short = 't', long)]
        tool: bool,

        /// Jump to entry ID ± 5 messages
        #[arg(long)]
        around: Option<String>,

        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// List sessions
    #[command(visible_alias = "l")]
    Ls {
        /// Days to search back (default: 7)
        #[arg(short = 's', long, default_value_t = 7.0)]
        since: f64,

        /// Max results (default: 20)
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,

        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// Tool calls across sessions
    #[command(visible_alias = "t")]
    Tools {
        /// Days to search back (default: 2)
        #[arg(short = 's', long, default_value_t = 2.0)]
        since: f64,

        /// Filter by tool name
        #[arg(short = 'n', long)]
        name: Option<String>,

        /// Filter by file path pattern (glob)
        #[arg(long)]
        file: Option<String>,

        /// Max results (default: 50)
        #[arg(short = 'l', long, default_value_t = 50)]
        limit: usize,

        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// Last N turns from the most recent session
    Last {
        /// User turns only
        #[arg(short = 'u', long)]
        user: bool,

        /// Assistant turns only
        #[arg(short = 'a', long)]
        assistant: bool,

        /// Tool calls only
        #[arg(short = 't', long)]
        tool: bool,

        /// Number of turns (default: 10)
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,

        /// JSON output
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Grep { pattern, user, assistant, tool, since, session, context, limit, json } => {
            commands::cmd_grep(
                pattern.as_deref(),
                user, assistant, tool,
                since,
                session.as_deref(),
                context, limit, json,
            );
        }
        Cmd::Show { session_id, user, assistant, tool, around, json } => {
            commands::cmd_show(
                &session_id,
                user, assistant, tool,
                around.as_deref(),
                json,
            );
        }
        Cmd::Ls { since, limit, json } => {
            commands::cmd_ls(since, limit, json);
        }
        Cmd::Tools { since, name, file, limit, json } => {
            commands::cmd_tools(
                since,
                name.as_deref(),
                file.as_deref(),
                limit, json,
            );
        }
        Cmd::Last { user, assistant, tool, limit, json } => {
            commands::cmd_last(user, assistant, tool, limit, json);
        }
    }
}
