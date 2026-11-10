//! Core data models for workspaces, floors, agents, messages, notes, and routines.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AgentState::Stopped => "stopped",
            AgentState::Starting => "starting",
            AgentState::Running => "running",
            AgentState::Busy => "busy",
            AgentState::WaitingQuota => "waiting_quota",
            AgentState::Exited => "exited",
            AgentState::Error => "error",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Ask,
    Notify,
    Broadcast,
    Reply,
}

impl std::fmt::Display for MessageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MessageKind::Ask => "ask",
            MessageKind::Notify => "notify",
            MessageKind::Broadcast => "broadcast",
            MessageKind::Reply => "reply",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Queued,
    Delivered,
    Answered,
    Expired,
    Failed,
}

impl std::fmt::Display for MessageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MessageState::Queued => "queued",
            MessageState::Delivered => "delivered",
            MessageState::Answered => "answered",
            MessageState::Expired => "expired",
            MessageState::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRef {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub state: AgentState,
    pub floor: Option<String>,
    pub preset: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteRef {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body: String,
    pub updated_at: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutineRef {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppStatus {
    pub version: String,
    pub workspaces: Vec<String>,
    pub agents_running: usize,
    pub queued_messages: usize,
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
}
