# `nexhub` command line

The CLI is a thin client of the running app (see ARCHITECTURE.md section 7). It reads `NEXHUB_PORT` and `NEXHUB_TOKEN` from the environment when it runs inside an agent terminal, otherwise `socket.json` and `token` from the data directory. Built with `clap` (derive). Output is human-readable by default and JSON with `--json`.

Exit codes: 0 ok, 1 usage error, 2 app not running, 3 unauthorized, 4 not found, 5 timeout, 6 conflict, 10 internal.

## Agents and messages

| Command | Does |
|---|---|
| `nexhub list [--workspace W]` | Agents with state, floor, preset, role. |
| `nexhub ask <agent> "<text>" [--timeout 30m]` | Sends an ask and waits; prints the reply body to stdout. |
| `nexhub notify <agent> "<text>"` | Sends a notify; prints the message id. |
| `nexhub broadcast "<text>"` | Notifies every agent of the caller's workspace. |
| `nexhub reply <message-id> "<text>"` | Answers an ask. |
| `nexhub inbox` | Messages addressed to the calling agent that are not answered. |
| `nexhub spawn <name> --preset P [--role R] [--floor F] [--workspace W]` | Starts an agent card. |
| `nexhub stop <agent>` | Stops the agent process (card stays). |

Text arguments may be `-` to read from stdin, or `@path` to read a file (useful for long briefs).

## Notes

`nexhub note get "<title>"`, `nexhub note set "<title>" "<body>|@file|-"`, `nexhub note list`.

## Floors

`nexhub floor new <name> [--base origin/develop] [--workspace W]`, `nexhub floor list`, `nexhub floor merge <name> --into develop`, `nexhub floor rm <name> [--force]`.

## Routines

`nexhub routine add <name> --every 45m --to <agent> "<prompt>"`, `--daily 08:00`, `--weekdays 09:30`, `--cron "<expr>"`, or `--run "<command>"` instead of `--to`. `nexhub routine list|rm <name>|run <name>`.

## Setup

- `nexhub init` in a repo: creates `.nexhub/roles/`, `.nexhub/presets/`, `.nexhub/briefs/` with the built-in presets copied, and registers the repo as a workspace (name = folder name, override with `--name`).
- `nexhub doctor`: checks git, agy, claude, gh, node on PATH, the app socket, token permissions; prints a table of found/missing with a hint per item.
- `nexhub status`: app version, workspaces, running agents, queued messages.

## Example: agents coordinating

```
# inside the reviewer terminal
nexhub ask builder "Is the PR for issue 4 ready? Reply with the URL."
# inside the builder terminal the envelope appears; the agent answers
nexhub reply 01JC... "https://github.com/ThiagoVenturim/askdash/pull/9"
```
