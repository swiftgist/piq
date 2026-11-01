# PIQ - Pi agent Query tool

This is a simple utility for your LLM and yourself to lookup past conversations and prevent the LLM from writing a throwaway script for the 79th time.  This _might_ work for other interfaces such as OpenCode, but I have not tested this at the moment.  Technically, your LLM is capable of writing this type of tool as well.  I did this as an experiment to see how well Qwen 3.8 Flash Next IQ3 would perform and how much hand holding this would need.

## Use case

My personal use case is when switching between phone, laptop and PC.  I can instruct the LLM to run 'piq last' and that's usually enough for a fresh session to pickup where the conversation was.  

Another use is when reviewing a larger project over several days and trying to pinpoint where in the discussion/instructions that progress went sideways.  

# Installation

```
cargo install --path .

```

Add the SKILL.md to your LLM.

# Usage

### grep
Search transcripts

Usage: piq grep [OPTIONS] [PATTERN]

Arguments:
  [PATTERN]  Search pattern (optional; omit to list recent turns)

Options:
  -u, --user               User turns only
  -a, --assistant          Assistant turns only
  -t, --tool               Tool calls/results only
      --compaction         Compaction summaries only
  -s, --since <SINCE>      Days to search back (default: 2) [default: 2]
  -S, --session <SESSION>  Restrict to one session (prefix match)
  -c, --context <CONTEXT>  Show N extra lines of context per match [default: 0]
  -l, --limit <LIMIT>      Max results (default: 30) [default: 30]
      --json               JSON output
  -h, --help               Print help

### show
Show a session

Usage: piq show [OPTIONS] <SESSION_ID>

Arguments:
  <SESSION_ID>  Session ID (or prefix)

Options:
  -u, --user             User turns only
  -a, --assistant        Assistant turns only
  -t, --tool             Tool calls only
      --around <AROUND>  Jump to entry ID ± 5 messages
      --json             JSON output
  -h, --help             Print help

### ls
List sessions, ordered by last activity

Usage: piq ls [OPTIONS]

Options:
  -s, --since <SINCE>  Days to search back, measured from last activity (default: 7) [default: 7]
  -n, --limit <LIMIT>  Max results (default: 20) [default: 20]
      --json           JSON output
  -h, --help           Print help

### tools
Tool calls across sessions

Usage: piq tools [OPTIONS]

Options:
  -s, --since <SINCE>  Days to search back (default: 2) [default: 2]
  -n, --name <NAME>    Filter by tool name
      --file <FILE>    Filter by file path pattern (glob)
  -l, --limit <LIMIT>  Max results (default: 50) [default: 50]
      --json           JSON output
  -h, --help           Print help

### compacts
List compaction/summary events (alias: c)

Usage: piq compacts [OPTIONS]

Options:
  -s, --since <SINCE>      Days to search back (default: 7) [default: 7]
  -S, --session <SESSION>  Restrict to one session (prefix match)
      --show               Print the full summary text for each event
  -l, --limit <LIMIT>      Max results (default: 30) [default: 30]
      --json               JSON output
  -h, --help               Print help

### last
Last N turns from the most recent session, skipping the session piq is running inside

Usage: piq last [OPTIONS]

Options:
  -u, --user           User turns only
  -a, --assistant      Assistant turns only
  -t, --tool           Tool calls only
  -n, --limit <LIMIT>  Number of turns (default: 10) [default: 10]
      --json           JSON output
  -h, --help           Print help
