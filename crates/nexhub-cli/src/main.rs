use clap::{Parser, Subcommand};

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

fn main() {
    let _cli = Cli::parse();
    println!("not implemented yet");
    std::process::exit(10);
}
