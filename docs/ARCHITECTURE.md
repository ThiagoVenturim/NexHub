# NexHub architecture

NexHub is a desktop app for running a team of coding agents (agy, Claude Code, plain shells, any CLI) side by side on one canvas, giving each one its own git worktree, and letting them talk to each other through a small message bus. This document is the contract every floor builds against. If code and this document disagree, fix one of them in the same PR.

## 1. Goals and non-goals

Goals for v0.1:
- Real terminals (ConPTY on Windows, PTY on macOS/Linux) shown as cards on an infinite canvas.
- Workspaces (one per repository) and groups in a sidebar.
- Floors: a git worktree plus a branch `layer/<name>`, created and removed by NexHub.
- Agent-to-agent messages: `ask` (wait for an answer), `notify` (fire and forget), `broadcast`.
- Roles and presets as versioned YAML files.
- Routines (scheduled prompts or commands) and a quota supervisor.
- A CLI, `nexhub`, that agents call from inside their terminals.

Non-goals for v0.1: cloud sync, multi-user, remote machines, browser automation, mobile app (the NexDeck bridge comes in v0.2).

## 2. Repository layout

```
nexhub/
  Cargo.toml                 cargo workspace: crates/*, src-tauri
  crates/
    nexhub-core/             pure library, no Tauri dependency
      src/lib.rs
      src/db.rs              SQLite schema, migrations, typed queries
      src/model.rs           Workspace, Floor, Agent, Message, Note, Routine, Run
      src/pty.rs             PtySession: spawn, write, resize, kill, output stream
      src/sessions.rs        SessionManager: agent_id -> PtySession, lifecycle
      src/floors.rs          git worktree create/list/merge/remove (shells out to git)
      src/roles.rs           YAML loader and validation for roles and presets
      src/bus.rs             message bus: enqueue, deliver, wait for reply
      src/routines.rs        scheduler (cron subset) and run log
      src/quota.rs           output watcher that detects quota states
      src/ipc.rs             local socket server, request/response types
      src/config.rs          paths, token, settings
    nexhub-cli/              binary `nexhub`, thin client over the socket
      src/main.rs
  src-tauri/                 Tauri 2 app, depends on nexhub-core
    src/main.rs
    src/commands.rs          #[tauri::command] wrappers over nexhub-core
    src/events.rs            backend -> UI event emitters
    tauri.conf.json
  ui/                        TypeScript + Vite, no framework lock-in (vanilla TS + small modules)
    src/main.ts
    src/canvas/              pan, zoom, card layout, connections
    src/cards/               terminal card (xterm.js), note card
    src/sidebar/
    src/api.ts               typed wrappers over Tauri invoke/listen
    src/styles/
    tests/                   vitest
  presets/                   built-in presets (agy.yaml, claude.yaml, shell.yaml)
  roles/                     example roles
  docs/
  scripts/check_no_emoji.py
  .githooks/
  .github/workflows/ci.yml
```

Rule: everything that can be tested without a window lives in `nexhub-core` and is covered by `cargo test`. `src-tauri` only adapts.

## 3. Data directory

- Windows: `%APPDATA%\NexHub\`, macOS: `~/Library/Application Support/NexHub/`, Linux: `$XDG_DATA_HOME/nexhub/`. Resolve with the `directories` crate. Override with env `NEXHUB_HOME` (tests always set it to a temp dir).
- Contents: `nexhub.db` (SQLite), `token` (32 random bytes, hex, created on first run, file permissions user-only), `socket.json` (`{"port": <u16>, "pid": <u32>}`), `logs/`.
- Per repository: `.nexhub/` at the repo root with `roles/*.yaml`, `presets/*.yaml`, `briefs/`. `nexhub init` creates it.

## 4. SQLite schema (migration 1)

Use `rusqlite` with the `bundled` feature. Migrations are numbered SQL strings in `db.rs`, tracked by `PRAGMA user_version`. Enable `PRAGMA journal_mode=WAL` and `foreign_keys=ON`. All ids are TEXT ULIDs (`ulid` crate). Timestamps are INTEGER unix milliseconds.

```sql
CREATE TABLE groups (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, repo_path TEXT NOT NULL,
  group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
  layout_json TEXT NOT NULL DEFAULT '{}',     -- canvas viewport and card positions
  created_at INTEGER NOT NULL
);
CREATE TABLE floors (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL, branch TEXT NOT NULL, path TEXT NOT NULL,
  base_ref TEXT NOT NULL,                     -- e.g. origin/develop
  status TEXT NOT NULL CHECK (status IN ('active','merged','removed')),
  created_at INTEGER NOT NULL, UNIQUE (workspace_id, name)
);
CREATE TABLE agents (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  floor_id TEXT REFERENCES floors(id) ON DELETE SET NULL,
  name TEXT NOT NULL,                         -- unique per workspace, used by `nexhub ask <name>`
  preset TEXT NOT NULL, role TEXT,            -- names resolved via roles.rs
  cwd TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('stopped','starting','running','busy','waiting_quota','exited','error')),
  quota_reset_at INTEGER,                     -- set by quota.rs when known
  last_prompt TEXT,                           -- last text delivered, used to resume after quota
  created_at INTEGER NOT NULL, UNIQUE (workspace_id, name)
);
CREATE TABLE messages (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_agent TEXT,                            -- NULL = the human or the CLI outside an agent
  to_agent TEXT,                              -- NULL = broadcast
  kind TEXT NOT NULL CHECK (kind IN ('ask','notify','broadcast','reply')),
  reply_to TEXT REFERENCES messages(id),
  body TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('queued','delivered','answered','expired','failed')),
  created_at INTEGER NOT NULL, delivered_at INTEGER, answered_at INTEGER
);
CREATE INDEX messages_inbox ON messages (to_agent, state);
CREATE TABLE notes (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL, body TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
  UNIQUE (workspace_id, title)
);
CREATE TABLE connections (                    -- edges drawn on the canvas; also an allowlist for ask/notify
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_id TEXT NOT NULL, to_id TEXT NOT NULL,  -- agent or note ids
  PRIMARY KEY (workspace_id, from_id, to_id)
);
CREATE TABLE routines (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL, schedule TEXT NOT NULL,  -- see section 8
  target_agent TEXT,                           -- prompt routine
  command TEXT,                                -- command routine (runs in workspace repo_path)
  body TEXT NOT NULL DEFAULT '', enabled INTEGER NOT NULL DEFAULT 1,
  next_run_at INTEGER, UNIQUE (workspace_id, name),
  CHECK ((target_agent IS NULL) <> (command IS NULL))
);
CREATE TABLE runs (
  id TEXT PRIMARY KEY, routine_id TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
  started_at INTEGER NOT NULL, finished_at INTEGER, exit_code INTEGER, output_tail TEXT
);
CREATE TABLE events (                         -- append-only audit log
  id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL,
  workspace_id TEXT, kind TEXT NOT NULL, data_json TEXT NOT NULL
);
```

## 5. PTY sessions (`pty.rs`, `sessions.rs`)

- Crate `portable-pty`. `PtySession::spawn(spec: SpawnSpec) -> Result<PtySession>` where `SpawnSpec { program: String, args: Vec<String>, cwd: PathBuf, env: Vec<(String,String)>, cols: u16, rows: u16 }`.
- Never pass a command line through a shell. The preset gives program and args as a list.
- Output is read on a dedicated thread into a `tokio::sync::broadcast::Sender<Vec<u8>>` (capacity 1024) and also into a ring buffer of the last 256 KiB so a card opened later can replay recent output.
- `write(&self, bytes)`, `resize(cols, rows)`, `kill()`, `wait() -> ExitStatus`.
- `SessionManager` holds `HashMap<AgentId, PtySession>` behind a `Mutex`, updates `agents.state`, and emits `agent-state` events.
- Delivering a prompt to an agent = write the text, then `\r`. For agy and Claude Code, wrap multi-line text in bracketed paste (`ESC[200~` ... `ESC[201~`) before `\r`.
- Busy detection v0.1: an agent is `busy` from delivery until 3 seconds without output; then `running` (idle). This is a heuristic and lives behind a trait `IdleDetector` so presets can override it later.

## 6. Message bus (`bus.rs`)

- `ask(from, to, body, timeout) -> Result<Reply>`: insert a `messages` row (`kind='ask'`, `state='queued'`), deliver it, then wait for a row with `reply_to = id`. Default timeout 30 minutes, max 24 hours.
- Delivery writes into the target PTY this envelope (plain text, no emoji):

```
[NexHub message <id> from <from_name>]
<body>
[Reply with: nexhub reply <id> "<your answer>"]
```

  For `notify` the last line is omitted.
- Queueing: if the target is `busy` or `waiting_quota`, the message stays `queued` and is delivered when the agent becomes idle. FIFO per target.
- `reply(id, body)` inserts `kind='reply'` and marks the ask `answered`, which wakes the waiting `ask`.
- `broadcast(from, body)`: one `notify` per agent in the workspace except the sender.
- Permission: an agent may `ask`/`notify` another agent of the same workspace. Cross-workspace messages are rejected in v0.1. The human (CLI outside an agent, or UI) may message anyone.

## 7. Local IPC (`ipc.rs`) and authentication

- Transport: TCP on `127.0.0.1` only, random port chosen at startup, written to `socket.json`. Never bind `0.0.0.0`.
- Framing: one JSON object per line (UTF-8, `\n`).
- Every request carries `"token"`. The server compares it with `subtle::ConstantTimeEq` against the token file. A missing or wrong token closes the connection with `{"ok":false,"error":{"code":"unauthorized"}}`. Fail closed: if the token file cannot be read, refuse everything.
- Agents get identity from env vars NexHub sets when it spawns them: `NEXHUB_AGENT_ID`, `NEXHUB_WORKSPACE_ID`, `NEXHUB_TOKEN`, `NEXHUB_PORT`. The CLI sends `"agent_id"` from the env; the server checks that the id exists and belongs to the workspace.
- Request: `{"id":"<client req id>","token":"...","agent_id":"...|null","method":"<name>","params":{...}}`.
- Response: `{"id":"...","ok":true,"result":{...}}` or `{"id":"...","ok":false,"error":{"code":"...","message":"..."}}`.
- Methods (params -> result):
  - `agents.list {workspace?}` -> `{agents:[{id,name,state,floor,preset,role}]}`
  - `agents.spawn {workspace, name, preset, role?, floor?}` -> `{agent}`
  - `agents.stop {name}` -> `{}`
  - `msg.ask {to, body, timeout_s?}` -> `{reply:{id, from, body}}` (blocks)
  - `msg.notify {to, body}` -> `{message_id}`
  - `msg.broadcast {body}` -> `{message_ids:[]}`
  - `msg.reply {message_id, body}` -> `{}`
  - `msg.inbox {}` -> `{messages:[]}` (for the calling agent)
  - `notes.get {title}` / `notes.set {title, body}` / `notes.list {}`
  - `floors.create {workspace, name, base_ref?}` / `floors.list {workspace}` / `floors.remove {workspace, name, force?}` / `floors.merge {workspace, name, into}`
  - `routines.list` / `routines.add {name, schedule, target? | command?, body}` / `routines.remove {name}` / `routines.run {name}`
  - `status {}` -> `{version, workspaces, agents_running, queued_messages}`
- Error codes: `unauthorized`, `not_found`, `invalid_params`, `conflict`, `timeout`, `forbidden`, `internal`.

## 8. Routines (`routines.rs`)

- Schedule grammar (subset, no full cron): `every <n>m`, `every <n>h`, `daily HH:MM`, `weekdays HH:MM`, `cron "<5-field expr>"` (use the `cron` crate only for this form).
- A tick task runs every 15 seconds, picks routines with `next_run_at <= now`, records a `runs` row, delivers the prompt (via bus as a `notify` from `routine:<name>`) or runs the command (no shell; split with `shell-words`, run in `repo_path`, capture last 8 KiB).
- Missed runs while the app was closed: run once on startup, not once per missed slot.

## 9. Quota supervisor (`quota.rs`)

- Watches each agent's output stream with patterns from its preset (`quota_patterns`, regexes). Built-in agy pattern includes `Waiting for quota reset` and `quota exceeded`; Claude Code preset includes `usage limit`.
- On match: state `waiting_quota`, store `last_prompt`, parse a reset time if the preset gives `quota_reset_regex` with a named group `at` (RFC3339) or `in_minutes`; otherwise retry every 30 minutes.
- On reset time: deliver `continue exactly where you stopped` followed by the stored `last_prompt` summary line, set state `busy`, log an event.

## 10. Floors (`floors.rs`)

- Create: `git -C <repo> fetch origin` (ignore failure offline), then `git -C <repo> worktree add -b layer/<name> <path> <base_ref>`. Path: `<repo_parent>/.nexhub-floors/<repo_name>--<name>`.
- If the branch exists, `git worktree add <path> layer/<name>` instead.
- Remove: refuse when `git status --porcelain` is not empty or the branch has commits not on its upstream, unless `force`. Never delete the branch.
- Merge: `git -C <repo> merge --no-ff layer/<name>` into `into` inside a temporary worktree; on conflict abort and return `conflict` with the file list. Never push.
- All git calls go through one function `run_git(dir, args) -> Result<Output>` using `std::process::Command` with args as a list.

## 11. Backend -> UI events (`src-tauri/src/events.rs`)

Emitted with `app.emit(name, payload)`; payloads are JSON:
- `pty-output` `{agent_id, data_b64}` (throttle: coalesce up to 16 ms)
- `agent-state` `{agent_id, state, quota_reset_at?}`
- `message` `{id, workspace_id, from, to, kind, body, state}`
- `note-changed` `{workspace_id, title}`
- `routine-run` `{routine_id, run_id, status}`
- `floor-changed` `{workspace_id, floor}`

UI -> backend uses Tauri commands with the same names and params as the IPC methods (`agents_spawn`, `msg_notify`, ...) plus `pty_write {agent_id, data_b64}`, `pty_resize {agent_id, cols, rows}`, `layout_save {workspace_id, layout_json}`.

## 12. Security rules (block the PR if violated)

- IPC only on loopback, token required, constant-time compare, fail closed.
- No shell anywhere: programs and args are lists; routine commands are split with `shell-words`, never `cmd /c` or `sh -c`.
- Paths from IPC (floor names, note titles) are validated: floor names match `^[a-z0-9][a-z0-9-]{0,40}$`; resolved floor paths must stay under the floors directory (`canonicalize` + `starts_with`).
- Tokens and env vars with `TOKEN`, `KEY`, `SECRET` in the name are never written to logs, events or the `events` table.
- Tauri: CSP set in `tauri.conf.json`, no remote URLs loaded, capabilities limited to the commands NexHub defines.

## 13. Testing

- `cargo test --workspace` must pass offline with `NEXHUB_HOME` set to a temp dir.
- PTY tests spawn a tiny portable program: on Windows `cmd.exe /d /q /k` is not allowed as a shell for prompts, so tests spawn `nexhub-core`'s own test helper binary `echo-agent` (a `[[bin]]` under `crates/nexhub-core/src/bin/echo_agent.rs` that echoes stdin lines prefixed with `echo: `).
- Bus tests use two echo agents and assert ask/reply round trips, queueing while busy and timeout.
- UI: `npm test` (vitest) for canvas math (pan/zoom transforms, hit testing), layout serialization and the API wrappers with a mocked `invoke`.
- CI matrix: windows-latest, ubuntu-latest, macos-latest; steps: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `npm ci && npm test` in `ui/`, `python scripts/check_no_emoji.py`.
