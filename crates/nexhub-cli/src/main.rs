use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::Value;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE_ERROR: i32 = 1;
pub const EXIT_APP_NOT_RUNNING: i32 = 2;
pub const EXIT_UNAUTHORIZED: i32 = 3;
pub const EXIT_NOT_FOUND: i32 = 4;
pub const EXIT_TIMEOUT: i32 = 5;
pub const EXIT_CONFLICT: i32 = 6;
pub const EXIT_INTERNAL: i32 = 10;

#[derive(Parser, Debug)]
#[command(name = "nexhub", version, about = "NexHub CLI")]
struct Cli {
    #[arg(long, global = true, help = "Output in JSON format")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "List agents with state, floor, preset, and role")]
    List {
        #[arg(long)]
        workspace: Option<String>,
    },
    #[command(about = "Send an ask to an agent and wait for a reply")]
    Ask {
        agent: String,
        text: String,
        #[arg(long, default_value = "30m")]
        timeout: String,
    },
    #[command(about = "Send a notification to an agent")]
    Notify { agent: String, text: String },
    #[command(about = "Notify every agent in the workspace")]
    Broadcast { text: String },
    #[command(about = "Answer an ask message")]
    Reply { message_id: String, text: String },
    #[command(about = "Check unanswered messages for the current agent")]
    Inbox,
    #[command(about = "Start an agent card")]
    Spawn {
        name: String,
        #[arg(long)]
        preset: String,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        floor: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
    },
    #[command(about = "Stop an agent process")]
    Stop { agent: String },
    #[command(about = "Manage notes")]
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },
    #[command(about = "Manage git worktree floors")]
    Floor {
        #[command(subcommand)]
        command: FloorCommands,
    },
    #[command(about = "Manage scheduled routines")]
    Routine {
        #[command(subcommand)]
        command: RoutineCommands,
    },
    #[command(about = "Initialize .nexhub in a repository")]
    Init {
        #[arg(long)]
        name: Option<String>,
    },
    #[command(about = "Check dependencies and system health")]
    Doctor,
    #[command(about = "Show application status")]
    Status,
}

#[derive(Subcommand, Debug)]
enum NoteCommands {
    #[command(about = "Get note contents")]
    Get { title: String },
    #[command(about = "Set note contents")]
    Set { title: String, body: String },
    #[command(about = "List notes")]
    List,
}

#[derive(Subcommand, Debug)]
enum FloorCommands {
    #[command(about = "Create a new floor worktree")]
    New {
        name: String,
        #[arg(long, default_value = "origin/develop")]
        base: String,
        #[arg(long)]
        workspace: Option<String>,
    },
    #[command(about = "List floors")]
    List {
        #[arg(long)]
        workspace: Option<String>,
    },
    #[command(about = "Merge a floor worktree into a target branch")]
    Merge {
        name: String,
        #[arg(long, default_value = "develop")]
        into: String,
    },
    #[command(about = "Remove a floor worktree")]
    Rm {
        name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum RoutineCommands {
    #[command(about = "Add a scheduled routine")]
    Add {
        name: String,
        #[arg(long)]
        every: Option<String>,
        #[arg(long)]
        daily: Option<String>,
        #[arg(long)]
        weekdays: Option<String>,
        #[arg(long)]
        cron: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        run: Option<String>,
        body: Option<String>,
    },
    #[command(about = "List routines")]
    List,
    #[command(about = "Remove a routine")]
    Rm { name: String },
    #[command(about = "Trigger a routine immediately")]
    Run { name: String },
}

fn resolve_text_arg(text: &str) -> Result<String, String> {
    if text == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        Ok(buf)
    } else if let Some(path) = text.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|e| format!("failed to read file '{path}': {e}"))
    } else {
        Ok(text.to_string())
    }
}

pub fn parse_timeout_s(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(1800);
    }
    if let Some(rest) = s.strip_suffix('m') {
        let m: u64 = rest
            .parse()
            .map_err(|_| format!("invalid timeout minutes: {s}"))?;
        Ok(m * 60)
    } else if let Some(rest) = s.strip_suffix('s') {
        let secs: u64 = rest
            .parse()
            .map_err(|_| format!("invalid timeout seconds: {s}"))?;
        Ok(secs)
    } else if let Some(rest) = s.strip_suffix('h') {
        let h: u64 = rest
            .parse()
            .map_err(|_| format!("invalid timeout hours: {s}"))?;
        Ok(h * 3600)
    } else {
        let secs: u64 = s
            .parse()
            .map_err(|_| format!("invalid timeout number: {s}"))?;
        Ok(secs)
    }
}

struct IpcClient {
    port: u16,
    token: String,
    agent_id: Option<String>,
}

impl IpcClient {
    fn from_env_or_discovery() -> Result<Self, i32> {
        let agent_id = std::env::var("NEXHUB_AGENT_ID").ok();

        let env_port = std::env::var("NEXHUB_PORT").ok();
        let env_token = std::env::var("NEXHUB_TOKEN").ok();

        if let (Some(p_str), Some(t)) = (env_port, env_token) {
            if let Ok(p) = p_str.parse::<u16>() {
                return Ok(Self {
                    port: p,
                    token: t,
                    agent_id,
                });
            }
        }

        let data_dir = nexhub_core::config::data_dir();
        let socket_info = match nexhub_core::config::read_socket_info(&data_dir) {
            Ok(info) => info,
            Err(_) => return Err(EXIT_APP_NOT_RUNNING),
        };

        let token = match nexhub_core::config::read_token(&data_dir) {
            Ok(t) => t,
            Err(_) => return Err(EXIT_APP_NOT_RUNNING),
        };

        Ok(Self {
            port: socket_info.port,
            token,
            agent_id,
        })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, (i32, String)> {
        let addr = format!("127.0.0.1:{}", self.port);
        let mut stream = TcpStream::connect(&addr)
            .map_err(|e| (EXIT_APP_NOT_RUNNING, format!("connection error: {e}")))?;

        let req = serde_json::json!({
            "id": "cli-req",
            "token": self.token,
            "agent_id": self.agent_id,
            "method": method,
            "params": params,
        });

        let mut req_bytes = serde_json::to_vec(&req)
            .map_err(|e| (EXIT_INTERNAL, format!("serialization error: {e}")))?;
        req_bytes.push(b'\n');

        stream
            .write_all(&req_bytes)
            .map_err(|e| (EXIT_APP_NOT_RUNNING, format!("write error: {e}")))?;

        let mut reader = std::io::BufReader::new(stream);
        let mut line = String::new();
        std::io::BufRead::read_line(&mut reader, &mut line)
            .map_err(|e| (EXIT_APP_NOT_RUNNING, format!("read error: {e}")))?;

        if line.is_empty() {
            return Err((
                EXIT_APP_NOT_RUNNING,
                "server closed connection unexpectedly".into(),
            ));
        }

        let resp: nexhub_core::ipc::IpcResponse = serde_json::from_str(&line)
            .map_err(|e| (EXIT_INTERNAL, format!("invalid response JSON: {e}")))?;

        if resp.ok {
            Ok(resp.result.unwrap_or(Value::Null))
        } else {
            let err = resp.error.unwrap_or(nexhub_core::ipc::IpcErrorPayload {
                code: "internal".into(),
                message: "unknown server error".into(),
            });
            let code = match err.code.as_str() {
                "unauthorized" => EXIT_UNAUTHORIZED,
                "not_found" => EXIT_NOT_FOUND,
                "timeout" => EXIT_TIMEOUT,
                "conflict" => EXIT_CONFLICT,
                "invalid_params" => EXIT_USAGE_ERROR,
                _ => EXIT_INTERNAL,
            };
            Err((code, err.message))
        }
    }
}

const PRESET_AGY: &str = include_str!("../../../presets/agy.yaml");
const PRESET_AGY_AUTO: &str = include_str!("../../../presets/agy-auto.yaml");
const PRESET_CLAUDE: &str = include_str!("../../../presets/claude.yaml");
const PRESET_SHELL: &str = include_str!("../../../presets/shell.yaml");

fn handle_init(name: Option<String>, json: bool) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let nexhub_dir = cwd.join(".nexhub");
    let roles_dir = nexhub_dir.join("roles");
    let presets_dir = nexhub_dir.join("presets");
    let briefs_dir = nexhub_dir.join("briefs");

    if let Err(e) = std::fs::create_dir_all(&roles_dir) {
        eprintln!("failed to create roles directory: {e}");
        return EXIT_INTERNAL;
    }
    if let Err(e) = std::fs::create_dir_all(&presets_dir) {
        eprintln!("failed to create presets directory: {e}");
        return EXIT_INTERNAL;
    }
    if let Err(e) = std::fs::create_dir_all(&briefs_dir) {
        eprintln!("failed to create briefs directory: {e}");
        return EXIT_INTERNAL;
    }

    let files = [
        ("agy.yaml", PRESET_AGY),
        ("agy-auto.yaml", PRESET_AGY_AUTO),
        ("claude.yaml", PRESET_CLAUDE),
        ("shell.yaml", PRESET_SHELL),
    ];

    for (fname, content) in files {
        let fpath = presets_dir.join(fname);
        if !fpath.exists() {
            if let Err(e) = std::fs::write(&fpath, content) {
                eprintln!("failed to write preset {fname}: {e}");
                return EXIT_INTERNAL;
            }
        }
    }

    let ws_name = name.unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace")
            .to_string()
    });

    if json {
        println!(
            "{}",
            serde_json::json!({
                "initialized": true,
                "workspace": ws_name,
                "path": nexhub_dir.display().to_string(),
            })
        );
    } else {
        println!(
            "Initialized .nexhub in {} (workspace: {ws_name})",
            cwd.display()
        );
    }

    EXIT_OK
}

fn check_tool(name: &str, hint: &str) -> (bool, String) {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let exts: Vec<&str> = if cfg!(windows) {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };

    let mut found = false;
    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }

    if found {
        (true, "found on PATH".to_string())
    } else {
        (false, hint.to_string())
    }
}

fn handle_doctor(json: bool) -> i32 {
    let tools = [
        ("git", "Install git and ensure it is on PATH"),
        ("agy", "Install Antigravity CLI and ensure it is on PATH"),
        ("claude", "Install Claude Code CLI and ensure it is on PATH"),
        ("gh", "Install GitHub CLI and ensure it is on PATH"),
        ("node", "Install Node.js and ensure it is on PATH"),
    ];

    let mut checks = Vec::new();
    for (name, hint) in tools {
        let (found, msg) = check_tool(name, hint);
        checks.push(serde_json::json!({
            "name": name,
            "found": found,
            "status": if found { "ok" } else { "missing" },
            "hint": msg,
        }));
    }

    // Check socket & token
    let data_dir = nexhub_core::config::data_dir();
    let socket_ok = nexhub_core::config::read_socket_info(&data_dir).is_ok();
    let token_ok = nexhub_core::config::read_token(&data_dir).is_ok();

    checks.push(serde_json::json!({
        "name": "socket",
        "found": socket_ok,
        "status": if socket_ok { "ok" } else { "not running" },
        "hint": if socket_ok { "app socket found" } else { "start NexHub app" },
    }));

    checks.push(serde_json::json!({
        "name": "token",
        "found": token_ok,
        "status": if token_ok { "ok" } else { "missing" },
        "hint": if token_ok { "app token found" } else { "run app to generate token" },
    }));

    if json {
        println!("{}", serde_json::json!({ "checks": checks }));
    } else {
        println!("{:<10} {:<12} Hint", "Tool/Item", "Status");
        println!("{}", "-".repeat(50));
        for c in checks {
            println!(
                "{:<10} {:<12} {}",
                c["name"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or(""),
                c["hint"].as_str().unwrap_or("")
            );
        }
    }

    EXIT_OK
}

fn run_command(client: &IpcClient, cli: &Cli) -> Result<(), (i32, String)> {
    match &cli.command {
        Commands::Init { .. } | Commands::Doctor => unreachable!(),
        Commands::List { workspace } => {
            let params = serde_json::json!({ "workspace": workspace });
            let res = client.call("agents.list", params)?;
            if cli.json {
                println!("{}", res);
            } else if let Some(agents) = res.get("agents").and_then(|v| v.as_array()) {
                println!(
                    "{:<15} {:<15} {:<15} {:<15} ROLE",
                    "NAME", "STATE", "FLOOR", "PRESET"
                );
                println!("{}", "-".repeat(70));
                for a in agents {
                    println!(
                        "{:<15} {:<15} {:<15} {:<15} {}",
                        a["name"].as_str().unwrap_or("-"),
                        a["state"].as_str().unwrap_or("-"),
                        a["floor"].as_str().unwrap_or("-"),
                        a["preset"].as_str().unwrap_or("-"),
                        a["role"].as_str().unwrap_or("-"),
                    );
                }
            } else {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Ask {
            agent,
            text,
            timeout,
        } => {
            let body = resolve_text_arg(text).map_err(|e| (EXIT_USAGE_ERROR, e))?;
            let timeout_s = parse_timeout_s(timeout).map_err(|e| (EXIT_USAGE_ERROR, e))?;
            let params = serde_json::json!({
                "to": agent,
                "body": body,
                "timeout_s": timeout_s,
            });
            let res = client.call("msg.ask", params)?;
            if cli.json {
                println!("{}", res);
            } else if let Some(reply_body) = res
                .get("reply")
                .and_then(|r| r.get("body"))
                .and_then(|b| b.as_str())
            {
                println!("{reply_body}");
            } else {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Notify { agent, text } => {
            let body = resolve_text_arg(text).map_err(|e| (EXIT_USAGE_ERROR, e))?;
            let params = serde_json::json!({
                "to": agent,
                "body": body,
            });
            let res = client.call("msg.notify", params)?;
            if cli.json {
                println!("{}", res);
            } else if let Some(msg_id) = res.get("message_id").and_then(|m| m.as_str()) {
                println!("{msg_id}");
            } else {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Broadcast { text } => {
            let body = resolve_text_arg(text).map_err(|e| (EXIT_USAGE_ERROR, e))?;
            let params = serde_json::json!({
                "body": body,
            });
            let res = client.call("msg.broadcast", params)?;
            if cli.json {
                println!("{}", res);
            } else if let Some(ids) = res.get("message_ids").and_then(|m| m.as_array()) {
                for id in ids {
                    if let Some(s) = id.as_str() {
                        println!("{s}");
                    }
                }
            } else {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Reply { message_id, text } => {
            let body = resolve_text_arg(text).map_err(|e| (EXIT_USAGE_ERROR, e))?;
            let params = serde_json::json!({
                "message_id": message_id,
                "body": body,
            });
            let res = client.call("msg.reply", params)?;
            if cli.json {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Inbox => {
            let res = client.call("msg.inbox", serde_json::json!({}))?;
            if cli.json {
                println!("{}", res);
            } else if let Some(msgs) = res.get("messages").and_then(|m| m.as_array()) {
                for m in msgs {
                    println!(
                        "[{}] From: {} Body: {}",
                        m["id"].as_str().unwrap_or("-"),
                        m["from_agent"].as_str().unwrap_or("human"),
                        m["body"].as_str().unwrap_or("")
                    );
                }
            } else {
                println!("{}", res);
            }
            Ok(())
        }
        Commands::Spawn {
            name,
            preset,
            role,
            floor,
            workspace,
        } => {
            let params = serde_json::json!({
                "name": name,
                "preset": preset,
                "role": role,
                "floor": floor,
                "workspace": workspace,
            });
            let res = client.call("agents.spawn", params)?;
            if cli.json {
                println!("{}", res);
            } else {
                println!("Spawned agent {name} ({preset})");
            }
            Ok(())
        }
        Commands::Stop { agent } => {
            let params = serde_json::json!({ "name": agent });
            let res = client.call("agents.stop", params)?;
            if cli.json {
                println!("{}", res);
            } else {
                println!("Stopped agent {agent}");
            }
            Ok(())
        }
        Commands::Note { command } => match command {
            NoteCommands::Get { title } => {
                let params = serde_json::json!({ "title": title });
                let res = client.call("notes.get", params)?;
                if cli.json {
                    println!("{}", res);
                } else if let Some(body) = res.get("body").and_then(|b| b.as_str()) {
                    println!("{body}");
                } else {
                    println!("{}", res);
                }
                Ok(())
            }
            NoteCommands::Set { title, body } => {
                let body_text = resolve_text_arg(body).map_err(|e| (EXIT_USAGE_ERROR, e))?;
                let params = serde_json::json!({ "title": title, "body": body_text });
                let res = client.call("notes.set", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Note saved: {title}");
                }
                Ok(())
            }
            NoteCommands::List => {
                let res = client.call("notes.list", serde_json::json!({}))?;
                if cli.json {
                    println!("{}", res);
                } else if let Some(notes) = res.get("notes").and_then(|n| n.as_array()) {
                    for n in notes {
                        println!("{}", n["title"].as_str().unwrap_or("-"));
                    }
                } else {
                    println!("{}", res);
                }
                Ok(())
            }
        },
        Commands::Floor { command } => match command {
            FloorCommands::New {
                name,
                base,
                workspace,
            } => {
                let params = serde_json::json!({
                    "name": name,
                    "base_ref": base,
                    "workspace": workspace,
                });
                let res = client.call("floors.create", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Created floor {name}");
                }
                Ok(())
            }
            FloorCommands::List { workspace } => {
                let params = serde_json::json!({ "workspace": workspace });
                let res = client.call("floors.list", params)?;
                if cli.json {
                    println!("{}", res);
                } else if let Some(floors) = res.get("floors").and_then(|f| f.as_array()) {
                    for f in floors {
                        println!(
                            "{:<20} {}",
                            f["name"].as_str().unwrap_or("-"),
                            f["status"].as_str().unwrap_or("-")
                        );
                    }
                } else {
                    println!("{}", res);
                }
                Ok(())
            }
            FloorCommands::Merge { name, into } => {
                let params = serde_json::json!({ "name": name, "into": into });
                let res = client.call("floors.merge", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Merged floor {name} into {into}");
                }
                Ok(())
            }
            FloorCommands::Rm { name, force } => {
                let params = serde_json::json!({ "name": name, "force": force });
                let res = client.call("floors.remove", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Removed floor {name}");
                }
                Ok(())
            }
        },
        Commands::Routine { command } => match command {
            RoutineCommands::Add {
                name,
                every,
                daily,
                weekdays,
                cron,
                to,
                run,
                body,
            } => {
                let schedule = if let Some(e) = every {
                    format!("every {e}")
                } else if let Some(d) = daily {
                    format!("daily {d}")
                } else if let Some(w) = weekdays {
                    format!("weekdays {w}")
                } else if let Some(c) = cron {
                    format!("cron \"{c}\"")
                } else {
                    return Err((
                        EXIT_USAGE_ERROR,
                        "one of --every, --daily, --weekdays, or --cron is required".into(),
                    ));
                };

                let body_resolved = if let Some(b) = body {
                    resolve_text_arg(b).map_err(|e| (EXIT_USAGE_ERROR, e))?
                } else {
                    String::new()
                };

                let params = serde_json::json!({
                    "name": name,
                    "schedule": schedule,
                    "target": to,
                    "command": run,
                    "body": body_resolved,
                });
                let res = client.call("routines.add", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Routine '{name}' added");
                }
                Ok(())
            }
            RoutineCommands::List => {
                let res = client.call("routines.list", serde_json::json!({}))?;
                if cli.json {
                    println!("{}", res);
                } else if let Some(routines) = res.get("routines").and_then(|r| r.as_array()) {
                    for r in routines {
                        println!(
                            "{:<20} {}",
                            r["name"].as_str().unwrap_or("-"),
                            r["schedule"].as_str().unwrap_or("-")
                        );
                    }
                } else {
                    println!("{}", res);
                }
                Ok(())
            }
            RoutineCommands::Rm { name } => {
                let params = serde_json::json!({ "name": name });
                let res = client.call("routines.remove", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Routine '{name}' removed");
                }
                Ok(())
            }
            RoutineCommands::Run { name } => {
                let params = serde_json::json!({ "name": name });
                let res = client.call("routines.run", params)?;
                if cli.json {
                    println!("{}", res);
                } else {
                    println!("Routine '{name}' triggered");
                }
                Ok(())
            }
        },
        Commands::Status => {
            let res = client.call("status", serde_json::json!({}))?;
            if cli.json {
                println!("{}", res);
            } else {
                println!("NexHub Status:");
                println!(
                    "  Version:          {}",
                    res["version"].as_str().unwrap_or("-")
                );
                println!(
                    "  Running agents:   {}",
                    res["agents_running"].as_u64().unwrap_or(0)
                );
                println!(
                    "  Queued messages:  {}",
                    res["queued_messages"].as_u64().unwrap_or(0)
                );
            }
            Ok(())
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            std::process::exit(handle_init(name, cli.json));
        }
        Commands::Doctor => {
            std::process::exit(handle_doctor(cli.json));
        }
        _ => {}
    }

    let client = match IpcClient::from_env_or_discovery() {
        Ok(c) => c,
        Err(code) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "ok": false, "error": { "code": "app_not_running", "message": "NexHub app is not running" } })
                );
            } else {
                eprintln!("NexHub app is not running (socket or token not found)");
            }
            std::process::exit(code);
        }
    };

    match run_command(&client, &cli) {
        Ok(()) => std::process::exit(EXIT_OK),
        Err((code, msg)) => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({ "ok": false, "error": { "code": code, "message": msg } })
                );
            } else {
                eprintln!("{msg}");
            }
            std::process::exit(code);
        }
    }
}
