# Roles and presets

A **preset** says how to start a program. A **role** says what the agent is for. An agent = one preset + optional role + optional floor. Both are YAML, loaded by `crates/nexhub-core/src/roles.rs` with `serde_yaml` and validated (unknown keys are an error).

Lookup order (first match wins): `<repo>/.nexhub/presets|roles/<name>.yaml`, then `<data dir>/presets|roles/`, then the built-ins compiled into the binary from `presets/` and `roles/` (`include_str!`).

## Preset schema

```yaml
name: agy                      # required, ^[a-z0-9-]+$
program: agy                   # required; resolved on PATH, or absolute path
args: ["--model", "gemini-3.8-flash"]   # list, never a shell string
env: {}                        # extra env; values may use ${repo}, ${floor}, ${data}
add_dirs_flag: "--add-dir"     # optional; NexHub appends <flag> <dir> for each role add_dirs entry
system_prompt_flag: null       # optional flag that takes a file path for the role prompt
first_message: role            # role | none: deliver the role prompt as the first message
quota_patterns: ["Waiting for quota reset", "(?i)quota exceeded"]
quota_reset_regex: null        # optional, named group `at` (RFC3339) or `in_minutes`
idle_ms: 3000                  # busy -> idle after this long without output
```

Built-ins to ship:
- `agy.yaml`: program `agy`, args `["--model","gemini-3.8-flash"]`, `add_dirs_flag: "--add-dir"`, agy quota patterns. A second built-in `agy-auto.yaml` adds `--dangerously-skip-permissions` and is labelled "full autonomy" in the UI.
- `claude.yaml`: program `claude`, args `[]`, `add_dirs_flag: "--add-dir"`, `system_prompt_flag: "--append-system-prompt-file"`, quota pattern `(?i)usage limit`.
- `shell.yaml`: program `powershell.exe` on Windows (`args: ["-NoLogo"]`), `$SHELL` elsewhere; no quota patterns; `first_message: none`.

## Role schema

```yaml
name: floor-builder            # required
title: Floor builder           # shown on the card
preset: agy-auto               # default preset when spawning with this role
add_dirs: ["${repo}"]
prompt: |                      # delivered as first message (or via system_prompt_flag)
  You are the builder of floor ${floor} in ${repo}.
  Read the brief in .nexhub/briefs/ named in your first task. Commit small, open a PR to develop,
  then run: nexhub notify conductor "pr_ready <url>"
budget:                        # optional, enforced by the quota supervisor in v0.2
  max_hours_per_day: 8
```

Variables available in strings: `${repo}` (workspace repo path), `${floor}` (floor name or empty), `${floor_path}`, `${agent}` (agent name), `${data}` (data dir).

## Validation errors (tests required for each)

- unknown key, missing `name` or `program`, `args` not a list, invalid regex in `quota_patterns`, role pointing to an unknown preset, variable that does not exist.
