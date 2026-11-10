//! Core data models for workspaces, floors, agents, messages, notes, and routines.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("unknown agent state: '{0}'")]
    UnknownAgentState(String),

    #[error("unknown message kind: '{0}'")]
    UnknownMessageKind(String),

    #[error("unknown message state: '{0}'")]
    UnknownMessageState(String),

    #[error("unknown floor status: '{0}'")]
    UnknownFloorStatus(String),
}

/// Agent lifecycle state matching the database check constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Stopped,
    Starting,
    Running,
    Busy,
    WaitingQuota,
    Exited,
    Error,
}

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Busy => "busy",
            Self::WaitingQuota => "waiting_quota",
            Self::Exited => "exited",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AgentState {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stopped" => Ok(Self::Stopped),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "busy" => Ok(Self::Busy),
            "waiting_quota" => Ok(Self::WaitingQuota),
            "exited" => Ok(Self::Exited),
            "error" => Ok(Self::Error),
            other => Err(ModelError::UnknownAgentState(other.to_string())),
        }
    }
}

/// Classification of messages passed through the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Ask,
    Notify,
    Broadcast,
    Reply,
}

impl MessageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Notify => "notify",
            Self::Broadcast => "broadcast",
            Self::Reply => "reply",
        }
    }
}

impl fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for MessageKind {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ask" => Ok(Self::Ask),
            "notify" => Ok(Self::Notify),
            "broadcast" => Ok(Self::Broadcast),
            "reply" => Ok(Self::Reply),
            other => Err(ModelError::UnknownMessageKind(other.to_string())),
        }
    }
}

/// Delivery and lifecycle status of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Queued,
    Delivered,
    Answered,
    Expired,
    Failed,
}

impl MessageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Delivered => "delivered",
            Self::Answered => "answered",
            Self::Expired => "expired",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for MessageState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for MessageState {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "delivered" => Ok(Self::Delivered),
            "answered" => Ok(Self::Answered),
            "expired" => Ok(Self::Expired),
            "failed" => Ok(Self::Failed),
            other => Err(ModelError::UnknownMessageState(other.to_string())),
        }
    }
}

/// Status of a development floor worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FloorStatus {
    Active,
    Merged,
    Removed,
}

impl FloorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Merged => "merged",
            Self::Removed => "removed",
        }
    }
}

impl fmt::Display for FloorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for FloorStatus {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "merged" => Ok(Self::Merged),
            "removed" => Ok(Self::Removed),
            other => Err(ModelError::UnknownFloorStatus(other.to_string())),
        }
    }
}

/// Group organizational unit in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub position: i64,
}

/// Workspace representing a git repository canvas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub repo_path: String,
    pub group_id: Option<String>,
    pub layout_json: String,
    pub created_at: i64,
}

/// Development floor representing an isolated worktree and branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floor {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    pub base_ref: String,
    pub status: FloorStatus,
    pub created_at: i64,
}

/// Agent instance running inside a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub workspace_id: String,
    pub floor_id: Option<String>,
    pub name: String,
    pub preset: String,
    pub role: Option<String>,
    pub cwd: String,
    pub state: AgentState,
    pub quota_reset_at: Option<i64>,
    pub last_prompt: Option<String>,
    pub created_at: i64,
}

pub type AgentRef = Agent;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub state: AgentState,
    pub floor: Option<String>,
    pub preset: String,
    pub role: Option<String>,
}

/// Message exchanged across agents or between human/CLI and agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub workspace_id: String,
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
    pub kind: MessageKind,
    pub reply_to: Option<String>,
    pub body: String,
    pub state: MessageState,
    pub created_at: i64,
    pub delivered_at: Option<i64>,
    pub answered_at: Option<i64>,
}

/// Persistent sticky note on the workspace canvas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body: String,
    pub updated_at: i64,
}

pub type NoteRef = Note;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FloorRef {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    pub base_ref: String,
    pub status: String,
    pub created_at: i64,
}

/// Canvas connection edge between agents and/or notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub workspace_id: String,
    pub from_id: String,
    pub to_id: String,
}

/// Scheduled routine configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub schedule: String,
    pub target_agent: Option<String>,
    pub command: Option<String>,
    pub body: String,
    pub enabled: bool,
    pub next_run_at: Option<i64>,
}

pub type RoutineRef = Routine;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppStatus {
    pub version: String,
    pub workspaces: Vec<String>,
    pub agents_running: usize,
    pub queued_messages: usize,
}

/// Execution record of a routine run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub routine_id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub output_tail: Option<String>,
}

/// Append-only audit log event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub at: i64,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub data_json: String,
}

/// Specification required to spawn a PTY process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub cols: u16,
    pub rows: u16,
}

impl Default for SpawnSpec {
    fn default() -> Self {
        Self {
            program: String::new(),
            args: Vec::new(),
            cwd: PathBuf::from("."),
            env: Vec::new(),
            cols: 80,
            rows: 24,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_serialization() {
        assert_eq!(
            serde_json::to_string(&AgentState::WaitingQuota).unwrap(),
            "\"waiting_quota\""
        );
        assert_eq!(serde_json::to_string(&MessageKind::Ask).unwrap(), "\"ask\"");
        assert_eq!(
            serde_json::to_string(&MessageState::Delivered).unwrap(),
            "\"delivered\""
        );
    }

    #[test]
    fn test_enums_roundtrip() {
        for state in [
            AgentState::Stopped,
            AgentState::Starting,
            AgentState::Running,
            AgentState::Busy,
            AgentState::WaitingQuota,
            AgentState::Exited,
            AgentState::Error,
        ] {
            assert_eq!(state.as_str().parse::<AgentState>().unwrap(), state);
        }

        for kind in [
            MessageKind::Ask,
            MessageKind::Notify,
            MessageKind::Broadcast,
            MessageKind::Reply,
        ] {
            assert_eq!(kind.as_str().parse::<MessageKind>().unwrap(), kind);
        }

        for state in [
            MessageState::Queued,
            MessageState::Delivered,
            MessageState::Answered,
            MessageState::Expired,
            MessageState::Failed,
        ] {
            assert_eq!(state.as_str().parse::<MessageState>().unwrap(), state);
        }

        for status in [
            FloorStatus::Active,
            FloorStatus::Merged,
            FloorStatus::Removed,
        ] {
            assert_eq!(status.as_str().parse::<FloorStatus>().unwrap(), status);
        }
    }

    #[test]
    fn test_enums_invalid_string() {
        assert!(AgentState::from_str("invalid").is_err());
        assert!(MessageKind::from_str("invalid").is_err());
        assert!(MessageState::from_str("invalid").is_err());
        assert!(FloorStatus::from_str("invalid").is_err());
    }
}
