---
name: piq-transcripts
description: When you need to find something a previous session said or did (past transcripts, "what did we decide", "where did we leave off", prior tool calls, earlier file edits), use the piq CLI instead of writing throwaway Python/grep against session JSONL.
---

# piq — Query Pi Session Transcripts

`piq` is a compiled Rust CLI (default install: `~/.cargo/bin/piq`). It indexes Pi's session
JSONL under `~/.pi/agent/sessions/*/*.jsonl`.

**Why it exists:** before `piq`, every session that needed history wrote a one-off Python
script. That cost startup time and memory, and the script was discarded. `piq` is the durable
replacement — single-digit milliseconds per search on a small window, roughly 10 ms and ~4 MB
resident across a 200-session corpus.

**Do not** write `python3 -c "import json..."` against `~/.pi/agent/sessions/`.
Reach for `piq` first. If `piq` can't do it, extend the source and rebuild.

## Commands

| Command | Purpose |
|---|---|
| `piq ls` | List recent sessions (last-active + started dates, ID, dir) |
| `piq g [PATTERN]` | Search all turns. Pattern optional |
| `piq s <ID>` | Dump one session |
| `piq t` | List tool calls across sessions |
| `piq last` | Last N turns of the most recent session **other than the one piq is running inside** (`-n N`, `-u/-a/-t` to filter role) |
| `piq c` / `piq compacts` | List compaction summaries across sessions (`--show` prints full text; `-s <days>` back-date, default 7) |

Short aliases: `g`=grep `s`=show `l`=ls `t`=tools `c`=compacts

## Flags

- `-u/--user`, `-a/--assistant`, `-t/--tool` — restrict which roles are searched.
  Omit all three to match everything.
- `-s/--since <days>` — lookback window, **measured from last activity (file mtime), not session
  start**. Defaults to 2 days; bump for older material (`-s 30`, `-s 90`). `piq ls` and
  `piq compacts` default to 7.
- `-S/--session <ID>` — restrict grep to one session (capital `-S`, not `-s`).
- `-c/--context <n>` — more lines per match.
- `-l/--limit <n>` — cap results.
- `--around <entry-id>` (`piq s` only) — jump to an entry ± 5 messages. **The key flag
  for long sessions** — never dump a whole session when you can anchor.
- `-n/--name <tool>` and `--file <pattern>` (`piq t`).
- `--json` — machine-readable output. Prefer this when you'll parse further. Timestamps in JSON
  stay **UTC** even though human-readable output is local.

All ordering is by mtime, not by the filename timestamp. See *Ordering* below.

## What it reads

Pi keeps one subdirectory per working directory under `~/.pi/agent/sessions/`, and `piq` searches
all of them. There is no scoping or exclusion flag — a search covers every project you have ever
run pi in. That is the intended behaviour for a transcript query tool, but it means a search
started in one project can surface material from another.

## Retrieval pattern (three steps)

```bash
piq g -u "kw"                    # 1. locate: get session + entry ID
piq s <sid> --around <eid>       # 2. zoom: read the neighborhood
piq t -s 7 --file "~/src/<dir>"  # 3. audit: what got touched
```

Session IDs are UUIDv7; the **8-char prefix is enough** in practice. Ambiguous prefixes resolve
to the most recent match.

### "What did I actually run?" — the record beats my memory

After a compaction, the agent's memory of its own earlier tool calls comes from the **digest**, not
from the transcript. Digests truncate tool-call lines and can omit whole actions. When the agent then
reconstructs "I created X earlier", it is generating a plausible narrative, and a filesystem check that
finds nothing is **evidence about the memory, not about the filesystem**.

`piq t` is the ground truth for that question, because it lists tool calls only — no prose, no thinking:

```bash
piq t -s 3 --file "<path>"      # every tool call that touched a path
piq t -s 3 -n bash              # every bash call in the window
piq g "<string>" -S <sid>       # then check WHICH block it lives in
```

**Thinking text is not a record.** A command string that appears only in a `thinking` block was
*planned or remembered*, not executed. `piq g` matches thinking, tool calls and results alike, so a
hit proves the string exists in the session, not that the command ran. Confirm the block type before
concluding an action happened — a session once reported that files it had never created were being
deleted by an unidentified process, on the strength of commands it had only thought about.

## Exact-name lookups: `piq g` is substring-only, case-insensitive, and regex syntax fails silently

`piq g` does literal, case-insensitive substring matching. There is no regex mode. Two distinct
failure modes:

- **Over-match.** `piq g "vera"` matches "se**vera**l" — and "several" appears constantly in
  transcripts, so a name search returns thousands of hits that *look* like evidence. Searching for
  a person's, host's, or agent's name by bare substring is never conclusive. The same pattern
  matches case-insensitively, so `VERA`, `vera` and `Vera` are the same query.
- **Silent under-match.** Passing regex syntax is not an error — it is treated as a literal
  string. `piq g "\bVera\b"` reports `No matches found` while a real word-boundary grep over the
  same corpus returns hits. A false negative that *looks like a clean negative* is the dangerous
  one: it ends the investigation instead of redirecting it.

So the "never grep the JSONL directly" rule has one exception: **word-boundary name searches**.
For those:

```bash
find ~/.pi/agent/sessions -name '*.jsonl' -print0 | xargs -0 grep -cE "\bVera\b"
```

Then bring `piq s <ID> --around <eid>` back for reading the neighborhood.

Use absolute paths or `find | xargs`. Pi names each session subdirectory after the working
directory with slashes replaced — `--var-home-<user>--`, `--var-home-<user>-src-<project>--` —
so a *relative* glob expands to arguments beginning with `--`, which grep parses as options.
The failure is silent unless you let stderr through.

Candidate extension (only when it earns its keep): `piq g --word` or `--regex`.

## Ordering: mtime, not start time

Session JSONL is append-only and `pi --continue` / `--resume` / `--session` append to the
**original** file, whose filename timestamp never moves. Sorting on the filename therefore ranks
a long-lived session by the day it happened to start:

```
<sid>   started day 1, 17:37 UTC   last entry day 3, 18:44 UTC   1096 lines
        (13:37 local             )  (14:44 local                )
```

That session *was* the day-3 conversation but would rank below sessions that started on day 2. `piq` sorts and windows on mtime instead, which matches the last entry's timestamp exactly.
Consequence: `-s N` correctly includes a session that was *used* recently even if it *started*
long ago — and a session genuinely idle for N days drops out even if it started yesterday.

### Timestamps are local in human output, UTC in `--json`

`ls` / `last` / `s` / `g` / `t` / `c` all render local time and print one note line:

```
# timestamps local: <zone> (<offset>)  |  --json stays UTC
```

`--json` is intentionally UTC because it is meant to be parsed, and UTC is the canonical clock
inside the JSONL. The zone name and offset come from your machine. The offset flips across
daylight saving — a filename reading `...T22-36-30Z` is `18:36` local in summer and `17:36` in
winter on a UTC-4 box — so **read the note line rather than assuming an offset**.

Gotcha for anyone extending this: chrono's `%Z` on a `DateTime<Local>` yields the numeric offset
(`-04:00`), **not** the abbreviation. The label comes from `date +%Z` instead, which also honours
`TZ` if it is overridden.

## Entry / schema notes

- Entry types: `session`, `model_change`, `thinking_level_change`, `custom`,
  `compaction`, `message`.
- Content block types: `text`, `thinking`, `toolCall`. It is **`toolCall`** (camelCase),
  *not* `tool_use` — grepping for `tool_use` returns nothing and is a known dead end.
- Role of tool output is `toolResult` (`piq` surfaces it as `tool`).
- `compaction` entries hold session summaries — useful "what was the plan" breadcrumbs.

## Extending

Edit `src/{main,parser,commands,model}.rs`, then either:

```bash
cargo install --path .        # reinstalls in place, replacing the existing binary
```

or, while iterating:

```bash
cd <src> && cargo build --release && cp target/release/piq ~/.cargo/bin/piq
```

Two things not to undo while editing:
- `main()` resets SIGPIPE to `SIG_DFL`. Rust ignores SIGPIPE at startup, so without it any
  `piq ... | head` panics with `Broken pipe` and exits 101.
- `discover_sessions()` sorts and windows on **mtime**. Switching back to the filename timestamp
  re-breaks resumed sessions (see *Ordering*).
