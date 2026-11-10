//! Message bus supporting ask, notify, broadcast, and reply between agents.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use crate::model::{AgentRef, AgentState, Message, MessageKind, MessageState};

pub const DEFAULT_ASK_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const MAX_ASK_TIMEOUT: Duration = Duration::from_secs(24 * 3600);

#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),
    #[error("message not found: {0}")]
    MessageNotFound(String),
    #[error("cross-workspace messaging is not allowed: {0}")]
    CrossWorkspaceForbidden(String),
    #[error("delivery error: {0}")]
    DeliveryFailed(String),
    #[error("ask timed out after {0:?}")]
    Timeout(Duration),
    #[error("invalid message state: {0}")]
    InvalidState(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("internal bus error: {0}")]
    Internal(String),
}

pub trait AgentDirectory: Send + Sync {
    fn find(&self, ws: &str, name: &str) -> Option<AgentRef>;
    fn list(&self, ws: &str) -> Vec<AgentRef>;
    fn state(&self, id: &str) -> AgentState;
    fn find_by_id(&self, _id: &str) -> Option<AgentRef> {
        None
    }
}

pub trait Deliverer: Send + Sync {
    fn deliver(&self, agent_id: &str, text: &str) -> Result<(), BusError>;
}

pub trait MessageStore: Send + Sync {
    fn insert(&self, message: &Message) -> Result<(), BusError>;
    fn update_state(&self, id: &str, state: MessageState, timestamp: i64) -> Result<(), BusError>;
    fn get(&self, id: &str) -> Result<Option<Message>, BusError>;
    fn next_queued_for(&self, agent_id: &str) -> Result<Option<Message>, BusError>;
    fn find_reply(&self, ask_id: &str) -> Result<Option<Message>, BusError>;
    fn inbox_for(&self, agent_id: &str) -> Result<Vec<Message>, BusError>;
    fn count_queued(&self) -> Result<usize, BusError>;
}

pub struct Bus<D: Deliverer, S: MessageStore, A: AgentDirectory> {
    deliverer: Arc<D>,
    store: Arc<S>,
    directory: Arc<A>,
    pending_replies: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Message>>>>,
}

impl<D: Deliverer, S: MessageStore, A: AgentDirectory> Bus<D, S, A> {
    pub fn new(deliverer: Arc<D>, store: Arc<S>, directory: Arc<A>) -> Self {
        Self {
            deliverer,
            store,
            directory,
            pending_replies: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn format_envelope(id: &str, from_name: &str, body: &str, is_ask: bool) -> String {
        if is_ask {
            format!(
                "[NexHub message {id} from {from_name}]\n{body}\n[Reply with: nexhub reply {id} \"<your answer>\"]"
            )
        } else {
            format!("[NexHub message {id} from {from_name}]\n{body}")
        }
    }

    fn resolve_sender_info(
        &self,
        from: Option<&str>,
    ) -> Result<(Option<String>, String, Option<String>), BusError> {
        match from {
            Some(from_id) => {
                if let Some(agent) = self.directory.find_by_id(from_id) {
                    Ok((Some(agent.id), agent.name, Some(agent.workspace_id)))
                } else if from_id.starts_with("routine:") {
                    Ok((Some(from_id.to_string()), from_id.to_string(), None))
                } else {
                    Err(BusError::AgentNotFound(from_id.to_string()))
                }
            }
            None => Ok((None, "human".to_string(), None)),
        }
    }

    fn resolve_target(&self, to_name: &str, ws_hint: Option<&str>) -> Result<AgentRef, BusError> {
        if let Some(ws) = ws_hint {
            self.directory
                .find(ws, to_name)
                .ok_or_else(|| BusError::AgentNotFound(to_name.to_string()))
        } else {
            if let Some(agent) = self.directory.find("", to_name) {
                return Ok(agent);
            }
            let list = self.directory.list("");
            for agent in list {
                if agent.name == to_name {
                    return Ok(agent);
                }
            }
            Err(BusError::AgentNotFound(to_name.to_string()))
        }
    }

    pub async fn ask(
        &self,
        from: Option<&str>,
        to_name: &str,
        body: &str,
        ws_hint: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<Message, BusError> {
        let (from_id, from_name, sender_ws) = self.resolve_sender_info(from)?;

        let target = self.resolve_target(to_name, ws_hint.or(sender_ws.as_deref()))?;

        if let Some(ref sws) = sender_ws {
            if sws != &target.workspace_id {
                return Err(BusError::CrossWorkspaceForbidden(format!(
                    "sender workspace {sws} != target workspace {}",
                    target.workspace_id
                )));
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        let message_id = ulid::Ulid::new().to_string();

        let message = Message {
            id: message_id.clone(),
            workspace_id: target.workspace_id.clone(),
            from_agent: from_id,
            to_agent: Some(target.id.clone()),
            kind: MessageKind::Ask,
            reply_to: None,
            body: body.to_string(),
            state: MessageState::Queued,
            created_at: now,
            delivered_at: None,
            answered_at: None,
        };

        self.store.insert(&message)?;

        let target_state = self.directory.state(&target.id);
        if target_state == AgentState::Running {
            let env = Self::format_envelope(&message_id, &from_name, body, true);
            self.deliverer.deliver(&target.id, &env)?;
            self.store
                .update_state(&message_id, MessageState::Delivered, now)?;
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending_replies.lock().unwrap();
            pending.insert(message_id.clone(), tx);
        }

        let mut wait_time = timeout.unwrap_or(DEFAULT_ASK_TIMEOUT);
        if wait_time > MAX_ASK_TIMEOUT {
            wait_time = MAX_ASK_TIMEOUT;
        }

        match tokio::time::timeout(wait_time, rx).await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(_)) => {
                if let Some(reply) = self.store.find_reply(&message_id)? {
                    Ok(reply)
                } else {
                    Err(BusError::Internal("reply channel closed".to_string()))
                }
            }
            Err(_) => {
                {
                    let mut pending = self.pending_replies.lock().unwrap();
                    pending.remove(&message_id);
                }
                let expired_at = chrono::Utc::now().timestamp_millis();
                let _ = self
                    .store
                    .update_state(&message_id, MessageState::Expired, expired_at);
                Err(BusError::Timeout(wait_time))
            }
        }
    }

    pub async fn notify(
        &self,
        from: Option<&str>,
        to_name: &str,
        body: &str,
        ws_hint: Option<&str>,
    ) -> Result<String, BusError> {
        let (from_id, from_name, sender_ws) = self.resolve_sender_info(from)?;

        let target = self.resolve_target(to_name, ws_hint.or(sender_ws.as_deref()))?;

        if let Some(ref sws) = sender_ws {
            if sws != &target.workspace_id {
                return Err(BusError::CrossWorkspaceForbidden(format!(
                    "sender workspace {sws} != target workspace {}",
                    target.workspace_id
                )));
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        let message_id = ulid::Ulid::new().to_string();

        let message = Message {
            id: message_id.clone(),
            workspace_id: target.workspace_id.clone(),
            from_agent: from_id,
            to_agent: Some(target.id.clone()),
            kind: MessageKind::Notify,
            reply_to: None,
            body: body.to_string(),
            state: MessageState::Queued,
            created_at: now,
            delivered_at: None,
            answered_at: None,
        };

        self.store.insert(&message)?;

        let target_state = self.directory.state(&target.id);
        if target_state == AgentState::Running {
            let env = Self::format_envelope(&message_id, &from_name, body, false);
            self.deliverer.deliver(&target.id, &env)?;
            self.store
                .update_state(&message_id, MessageState::Delivered, now)?;
        }

        Ok(message_id)
    }

    pub async fn broadcast(
        &self,
        from: Option<&str>,
        workspace: &str,
        body: &str,
    ) -> Result<Vec<String>, BusError> {
        let (from_id, from_name, sender_ws) = self.resolve_sender_info(from)?;

        if let Some(ref sws) = sender_ws {
            if sws != workspace {
                return Err(BusError::CrossWorkspaceForbidden(format!(
                    "sender workspace {sws} != target workspace {workspace}"
                )));
            }
        }

        let agents = self.directory.list(workspace);
        let mut ids = Vec::new();

        for target in agents {
            if let Some(ref fid) = from_id {
                if fid == &target.id {
                    continue;
                }
            }

            let now = chrono::Utc::now().timestamp_millis();
            let message_id = ulid::Ulid::new().to_string();

            let message = Message {
                id: message_id.clone(),
                workspace_id: workspace.to_string(),
                from_agent: from_id.clone(),
                to_agent: Some(target.id.clone()),
                kind: MessageKind::Broadcast,
                reply_to: None,
                body: body.to_string(),
                state: MessageState::Queued,
                created_at: now,
                delivered_at: None,
                answered_at: None,
            };

            self.store.insert(&message)?;

            let target_state = self.directory.state(&target.id);
            if target_state == AgentState::Running {
                let env = Self::format_envelope(&message_id, &from_name, body, false);
                let _ = self.deliverer.deliver(&target.id, &env);
                let _ = self
                    .store
                    .update_state(&message_id, MessageState::Delivered, now);
            }

            ids.push(message_id);
        }

        Ok(ids)
    }

    pub async fn reply(
        &self,
        reply_to_id: &str,
        from: Option<&str>,
        body: &str,
    ) -> Result<String, BusError> {
        let original = self
            .store
            .get(reply_to_id)?
            .ok_or_else(|| BusError::MessageNotFound(reply_to_id.to_string()))?;

        if original.kind != MessageKind::Ask {
            return Err(BusError::InvalidState(format!(
                "message {reply_to_id} is not an ask (kind={})",
                original.kind
            )));
        }

        let now = chrono::Utc::now().timestamp_millis();
        let reply_id = ulid::Ulid::new().to_string();

        let reply_message = Message {
            id: reply_id.clone(),
            workspace_id: original.workspace_id.clone(),
            from_agent: from.map(|s| s.to_string()),
            to_agent: original.from_agent.clone(),
            kind: MessageKind::Reply,
            reply_to: Some(reply_to_id.to_string()),
            body: body.to_string(),
            state: MessageState::Answered,
            created_at: now,
            delivered_at: Some(now),
            answered_at: Some(now),
        };

        self.store.insert(&reply_message)?;
        self.store
            .update_state(reply_to_id, MessageState::Answered, now)?;

        let sender = {
            let mut pending = self.pending_replies.lock().unwrap();
            pending.remove(reply_to_id)
        };

        if let Some(tx) = sender {
            let _ = tx.send(reply_message);
        }

        Ok(reply_id)
    }

    pub async fn inbox(&self, agent_id: &str) -> Result<Vec<Message>, BusError> {
        self.store.inbox_for(agent_id)
    }

    pub fn on_agent_idle(&self, agent_id: &str) -> Result<Option<String>, BusError> {
        if let Some(msg) = self.store.next_queued_for(agent_id)? {
            let sender_name = match msg.from_agent.as_deref() {
                Some(fid) => self
                    .directory
                    .find_by_id(fid)
                    .map(|a| a.name)
                    .unwrap_or_else(|| "unknown".to_string()),
                None => "human".to_string(),
            };

            let is_ask = msg.kind == MessageKind::Ask;
            let env = Self::format_envelope(&msg.id, &sender_name, &msg.body, is_ask);
            self.deliverer.deliver(agent_id, &env)?;

            let now = chrono::Utc::now().timestamp_millis();
            self.store
                .update_state(&msg.id, MessageState::Delivered, now)?;
            Ok(Some(msg.id))
        } else {
            Ok(None)
        }
    }
}

#[derive(Default)]
pub struct InMemoryDeliverer {
    pub deliveries: Mutex<Vec<(String, String)>>,
}

impl Deliverer for InMemoryDeliverer {
    fn deliver(&self, agent_id: &str, text: &str) -> Result<(), BusError> {
        let mut d = self.deliveries.lock().unwrap();
        d.push((agent_id.to_string(), text.to_string()));
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryAgentDirectory {
    pub agents: RwLock<HashMap<String, AgentRef>>,
}

impl InMemoryAgentDirectory {
    pub fn add(&self, agent: AgentRef) {
        let mut a = self.agents.write().unwrap();
        a.insert(agent.id.clone(), agent);
    }

    pub fn set_state(&self, id: &str, state: AgentState) {
        let mut a = self.agents.write().unwrap();
        if let Some(ag) = a.get_mut(id) {
            ag.state = state;
        }
    }
}

impl AgentDirectory for InMemoryAgentDirectory {
    fn find(&self, ws: &str, name: &str) -> Option<AgentRef> {
        let a = self.agents.read().unwrap();
        a.values()
            .find(|agent| (ws.is_empty() || agent.workspace_id == ws) && agent.name == name)
            .cloned()
    }

    fn find_by_id(&self, id: &str) -> Option<AgentRef> {
        let a = self.agents.read().unwrap();
        a.get(id).cloned()
    }

    fn list(&self, ws: &str) -> Vec<AgentRef> {
        let a = self.agents.read().unwrap();
        a.values()
            .filter(|agent| ws.is_empty() || agent.workspace_id == ws)
            .cloned()
            .collect()
    }

    fn state(&self, id: &str) -> AgentState {
        let a = self.agents.read().unwrap();
        a.get(id).map(|ag| ag.state).unwrap_or(AgentState::Stopped)
    }
}

#[derive(Default)]
pub struct InMemoryMessageStore {
    pub messages: RwLock<HashMap<String, Message>>,
    pub order: RwLock<Vec<String>>,
}

impl MessageStore for InMemoryMessageStore {
    fn insert(&self, message: &Message) -> Result<(), BusError> {
        let mut map = self.messages.write().unwrap();
        let mut ord = self.order.write().unwrap();
        map.insert(message.id.clone(), message.clone());
        ord.push(message.id.clone());
        Ok(())
    }

    fn update_state(&self, id: &str, state: MessageState, timestamp: i64) -> Result<(), BusError> {
        let mut map = self.messages.write().unwrap();
        if let Some(msg) = map.get_mut(id) {
            msg.state = state;
            match state {
                MessageState::Delivered => msg.delivered_at = Some(timestamp),
                MessageState::Answered => msg.answered_at = Some(timestamp),
                _ => {}
            }
            Ok(())
        } else {
            Err(BusError::MessageNotFound(id.to_string()))
        }
    }

    fn get(&self, id: &str) -> Result<Option<Message>, BusError> {
        let map = self.messages.read().unwrap();
        Ok(map.get(id).cloned())
    }

    fn next_queued_for(&self, agent_id: &str) -> Result<Option<Message>, BusError> {
        let map = self.messages.read().unwrap();
        let ord = self.order.read().unwrap();
        for id in ord.iter() {
            if let Some(msg) = map.get(id) {
                if msg.to_agent.as_deref() == Some(agent_id) && msg.state == MessageState::Queued {
                    return Ok(Some(msg.clone()));
                }
            }
        }
        Ok(None)
    }

    fn find_reply(&self, ask_id: &str) -> Result<Option<Message>, BusError> {
        let map = self.messages.read().unwrap();
        Ok(map
            .values()
            .find(|m| m.reply_to.as_deref() == Some(ask_id) && m.kind == MessageKind::Reply)
            .cloned())
    }

    fn inbox_for(&self, agent_id: &str) -> Result<Vec<Message>, BusError> {
        let map = self.messages.read().unwrap();
        let ord = self.order.read().unwrap();
        let mut res = Vec::new();
        for id in ord.iter() {
            if let Some(msg) = map.get(id) {
                if msg.to_agent.as_deref() == Some(agent_id)
                    && (msg.state == MessageState::Delivered || msg.state == MessageState::Queued)
                {
                    res.push(msg.clone());
                }
            }
        }
        Ok(res)
    }

    fn count_queued(&self) -> Result<usize, BusError> {
        let map = self.messages.read().unwrap();
        Ok(map
            .values()
            .filter(|m| m.state == MessageState::Queued)
            .count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bus() -> (
        Bus<InMemoryDeliverer, InMemoryMessageStore, InMemoryAgentDirectory>,
        Arc<InMemoryDeliverer>,
        Arc<InMemoryMessageStore>,
        Arc<InMemoryAgentDirectory>,
    ) {
        let deliverer = Arc::new(InMemoryDeliverer::default());
        let store = Arc::new(InMemoryMessageStore::default());
        let directory = Arc::new(InMemoryAgentDirectory::default());

        let bus = Bus::new(deliverer.clone(), store.clone(), directory.clone());
        (bus, deliverer, store, directory)
    }

    #[tokio::test]
    async fn test_ask_reply_roundtrip() {
        let (bus, deliverer, _store, dir) = create_test_bus();
        let bus = Arc::new(bus);

        let sender = AgentRef {
            id: "agent-1".to_string(),
            workspace_id: "ws-main".to_string(),
            floor_id: None,
            name: "reviewer".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        let target = AgentRef {
            id: "agent-2".to_string(),
            workspace_id: "ws-main".to_string(),
            floor_id: None,
            name: "builder".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };

        dir.add(sender);
        dir.add(target);

        let bus_clone = bus.clone();
        let ask_handle = tokio::spawn(async move {
            bus_clone
                .ask(
                    Some("agent-1"),
                    "builder",
                    "Is PR ready?",
                    Some("ws-main"),
                    Some(Duration::from_secs(2)),
                )
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let d = deliverer.deliveries.lock().unwrap();
        assert_eq!(d.len(), 1);
        let (agent_id, envelope) = &d[0];
        assert_eq!(agent_id, "agent-2");
        assert!(envelope.contains("[NexHub message "));
        assert!(envelope.contains("from reviewer]"));
        assert!(envelope.contains("Is PR ready?"));
        assert!(envelope.contains("[Reply with: nexhub reply "));

        let msg_id = envelope
            .split("[NexHub message ")
            .nth(1)
            .unwrap()
            .split(" from")
            .next()
            .unwrap()
            .to_string();

        drop(d);

        bus.reply(&msg_id, Some("agent-2"), "PR #42 is ready")
            .await
            .unwrap();

        let reply = ask_handle.await.unwrap().unwrap();
        assert_eq!(reply.body, "PR #42 is ready");
        assert_eq!(reply.kind, MessageKind::Reply);
    }

    #[tokio::test]
    async fn test_queue_while_busy_and_deliver_on_idle() {
        let (bus, deliverer, _store, dir) = create_test_bus();

        let target = AgentRef {
            id: "agent-busy".to_string(),
            workspace_id: "ws-main".to_string(),
            floor_id: None,
            name: "busy-agent".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Busy,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        dir.add(target);

        let msg_id = bus
            .notify(None, "busy-agent", "take a look later", Some("ws-main"))
            .await
            .unwrap();

        assert_eq!(deliverer.deliveries.lock().unwrap().len(), 0);

        dir.set_state("agent-busy", AgentState::Running);
        let delivered_id = bus.on_agent_idle("agent-busy").unwrap();
        assert_eq!(delivered_id, Some(msg_id));

        let d = deliverer.deliveries.lock().unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "agent-busy");
        assert!(d[0].1.contains("take a look later"));
    }

    #[tokio::test]
    async fn test_ask_timeout() {
        let (bus, _deliverer, store, dir) = create_test_bus();

        let target = AgentRef {
            id: "agent-lazy".to_string(),
            workspace_id: "ws-main".to_string(),
            floor_id: None,
            name: "lazy-agent".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        dir.add(target);

        let res = bus
            .ask(
                None,
                "lazy-agent",
                "hello",
                Some("ws-main"),
                Some(Duration::from_millis(50)),
            )
            .await;

        match res {
            Err(BusError::Timeout(_)) => {}
            other => panic!("expected timeout, got: {other:?}"),
        }

        let map = store.messages.read().unwrap();
        let msg = map.values().next().unwrap();
        assert_eq!(msg.state, MessageState::Expired);
    }

    #[tokio::test]
    async fn test_broadcast_does_not_deliver_to_sender() {
        let (bus, deliverer, _store, dir) = create_test_bus();

        let agent1 = AgentRef {
            id: "a1".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "agent-1".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        let agent2 = AgentRef {
            id: "a2".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "agent-2".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        let agent3 = AgentRef {
            id: "a3".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "agent-3".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };

        dir.add(agent1);
        dir.add(agent2);
        dir.add(agent3);

        let ids = bus
            .broadcast(Some("a1"), "ws-1", "Standup time")
            .await
            .unwrap();
        assert_eq!(ids.len(), 2);

        let d = deliverer.deliveries.lock().unwrap();
        assert_eq!(d.len(), 2);
        let delivered_targets: Vec<String> = d.iter().map(|(target, _)| target.clone()).collect();
        assert!(!delivered_targets.contains(&"a1".to_string()));
        assert!(delivered_targets.contains(&"a2".to_string()));
        assert!(delivered_targets.contains(&"a3".to_string()));
    }

    #[tokio::test]
    async fn test_cross_workspace_rejected() {
        let (bus, _deliverer, _store, dir) = create_test_bus();

        let agent_ws1 = AgentRef {
            id: "a-ws1".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "alice".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };
        let agent_ws2 = AgentRef {
            id: "a-ws2".to_string(),
            workspace_id: "ws-2".to_string(),
            floor_id: None,
            name: "bob".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        };

        dir.add(agent_ws1);
        dir.add(agent_ws2);

        let res = bus
            .ask(
                Some("a-ws1"),
                "bob",
                "hello",
                Some("ws-2"),
                Some(Duration::from_secs(1)),
            )
            .await;

        match res {
            Err(BusError::CrossWorkspaceForbidden(_)) => {}
            other => panic!("expected cross workspace forbidden error, got: {other:?}"),
        }
    }
}
