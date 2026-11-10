//! Local IPC socket server, request/response protocols, and authentication.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::bus::{AgentDirectory, Bus, BusError, Deliverer, MessageStore};
use crate::config;
use crate::model::{AgentSummary, AppStatus, FloorRef, NoteRef, RoutineRef};

pub const MAX_LINE_BYTES: usize = 1024 * 1024; // 1 MiB

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl IpcError {
    pub fn code(&self) -> &'static str {
        match self {
            IpcError::Unauthorized => "unauthorized",
            IpcError::NotFound(_) => "not_found",
            IpcError::InvalidParams(_) => "invalid_params",
            IpcError::Conflict(_) => "conflict",
            IpcError::Timeout(_) => "timeout",
            IpcError::Forbidden(_) => "forbidden",
            IpcError::Internal(_) => "internal",
        }
    }
}

impl From<BusError> for IpcError {
    fn from(err: BusError) -> Self {
        match err {
            BusError::AgentNotFound(s) => IpcError::NotFound(s),
            BusError::WorkspaceNotFound(s) => IpcError::NotFound(s),
            BusError::MessageNotFound(s) => IpcError::NotFound(s),
            BusError::CrossWorkspaceForbidden(s) => IpcError::Forbidden(s),
            BusError::Timeout(d) => IpcError::Timeout(format!("timed out after {d:?}")),
            BusError::DeliveryFailed(s) => IpcError::Internal(s),
            BusError::InvalidState(s) => IpcError::InvalidParams(s),
            BusError::Storage(s) => IpcError::Internal(s),
            BusError::Internal(s) => IpcError::Internal(s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub id: Option<String>,
    pub token: String,
    pub agent_id: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcErrorPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcErrorPayload {
    pub code: String,
    pub message: String,
}

impl IpcResponse {
    pub fn ok(id: Option<String>, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Option<String>, code: &str, message: &str) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(IpcErrorPayload {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

pub trait Control: Send + Sync {
    fn spawn_agent(
        &self,
        workspace: &str,
        name: &str,
        preset: &str,
        role: Option<&str>,
        floor: Option<&str>,
    ) -> impl std::future::Future<Output = Result<AgentSummary, IpcError>> + Send;

    fn stop_agent(
        &self,
        name: &str,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), IpcError>> + Send;

    fn list_agents(
        &self,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<AgentSummary>, IpcError>> + Send;

    fn get_note(
        &self,
        title: &str,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<NoteRef, IpcError>> + Send;

    fn set_note(
        &self,
        title: &str,
        body: &str,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<NoteRef, IpcError>> + Send;

    fn list_notes(
        &self,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<NoteRef>, IpcError>> + Send;

    fn create_floor(
        &self,
        workspace: &str,
        name: &str,
        base_ref: Option<&str>,
    ) -> impl std::future::Future<Output = Result<FloorRef, IpcError>> + Send;

    fn list_floors(
        &self,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<FloorRef>, IpcError>> + Send;

    fn remove_floor(
        &self,
        workspace: &str,
        name: &str,
        force: bool,
    ) -> impl std::future::Future<Output = Result<(), IpcError>> + Send;

    fn merge_floor(
        &self,
        workspace: &str,
        name: &str,
        into: &str,
    ) -> impl std::future::Future<Output = Result<(), IpcError>> + Send;

    fn status(&self) -> impl std::future::Future<Output = Result<AppStatus, IpcError>> + Send;
}

pub trait RoutineControl: Send + Sync {
    fn list_routines(
        &self,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<RoutineRef>, IpcError>> + Send;

    fn add_routine(
        &self,
        name: &str,
        schedule: &str,
        target: Option<&str>,
        command: Option<&str>,
        body: Option<&str>,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<RoutineRef, IpcError>> + Send;

    fn remove_routine(
        &self,
        name: &str,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), IpcError>> + Send;

    fn run_routine(
        &self,
        name: &str,
        workspace: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), IpcError>> + Send;
}

pub fn constant_time_eq_token(expected: &str, provided: &str) -> bool {
    let exp = expected.as_bytes();
    let prov = provided.as_bytes();
    if exp.len() != prov.len() {
        return false;
    }
    exp.ct_eq(prov).into()
}

pub struct IpcServer<C, D, S, A, R>
where
    C: Control + 'static,
    D: Deliverer + 'static,
    S: MessageStore + 'static,
    A: AgentDirectory + 'static,
    R: RoutineControl + 'static,
{
    data_dir: PathBuf,
    control: Arc<C>,
    bus: Arc<Bus<D, S, A>>,
    routines: Arc<R>,
    directory: Arc<A>,
    shutdown_tx: broadcast::Sender<()>,
}

impl<C, D, S, A, R> IpcServer<C, D, S, A, R>
where
    C: Control + 'static,
    D: Deliverer + 'static,
    S: MessageStore + 'static,
    A: AgentDirectory + 'static,
    R: RoutineControl + 'static,
{
    pub fn new(
        data_dir: PathBuf,
        control: Arc<C>,
        bus: Arc<Bus<D, S, A>>,
        routines: Arc<R>,
        directory: Arc<A>,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            data_dir,
            control,
            bus,
            routines,
            directory,
            shutdown_tx,
        }
    }

    pub async fn start(&self) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), std::io::Error> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        if !addr.ip().is_loopback() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "listener must only bind to loopback address",
            ));
        }

        config::write_socket_info(&self.data_dir, addr.port(), std::process::id())?;

        let data_dir = self.data_dir.clone();
        let control = self.control.clone();
        let bus = self.bus.clone();
        let routines = self.routines.clone();
        let directory = self.directory.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, _peer_addr)) => {
                                let data_dir = data_dir.clone();
                                let control = control.clone();
                                let bus = bus.clone();
                                let routines = routines.clone();
                                let directory = directory.clone();
                                tokio::spawn(async move {
                                    let _ = Self::handle_connection(stream, data_dir, control, bus, routines, directory).await;
                                });
                            }
                            Err(e) => {
                                tracing::warn!("failed to accept connection: {e}");
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
            let _ = config::remove_socket_info(&data_dir);
        });

        Ok((addr, handle))
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    async fn handle_connection(
        stream: TcpStream,
        data_dir: PathBuf,
        control: Arc<C>,
        bus: Arc<Bus<D, S, A>>,
        routines: Arc<R>,
        directory: Arc<A>,
    ) -> Result<(), std::io::Error> {
        let (mut reader, mut writer) = stream.into_split();
        let mut unparsed = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            if let Some(pos) = unparsed.iter().position(|&b| b == b'\n') {
                if pos > MAX_LINE_BYTES {
                    let err_resp = IpcResponse::err(
                        None,
                        "invalid_params",
                        "request line exceeds 1 MiB limit",
                    );
                    let mut data = serde_json::to_vec(&err_resp)?;
                    data.push(b'\n');
                    let _ = writer.write_all(&data).await;
                    let _ = writer.flush().await;
                    return Ok(());
                }

                let line_bytes: Vec<u8> = unparsed.drain(..=pos).collect();
                let raw_str = match std::str::from_utf8(&line_bytes) {
                    Ok(s) => s.trim_end(),
                    Err(_) => {
                        let err_resp =
                            IpcResponse::err(None, "invalid_params", "invalid utf-8 line");
                        let mut data = serde_json::to_vec(&err_resp)?;
                        data.push(b'\n');
                        let _ = writer.write_all(&data).await;
                        let _ = writer.flush().await;
                        return Ok(());
                    }
                };

                if raw_str.trim().is_empty() {
                    continue;
                }

                let req: IpcRequest = match serde_json::from_str(raw_str) {
                    Ok(r) => r,
                    Err(e) => {
                        let err_resp = IpcResponse::err(
                            None,
                            "invalid_params",
                            &format!("invalid JSON request: {e}"),
                        );
                        let mut data = serde_json::to_vec(&err_resp)?;
                        data.push(b'\n');
                        let _ = writer.write_all(&data).await;
                        let _ = writer.flush().await;
                        return Ok(());
                    }
                };

                let expected_token = match config::read_token(&data_dir) {
                    Ok(t) => t,
                    Err(_) => {
                        let err_resp = IpcResponse::err(
                            req.id,
                            "unauthorized",
                            "cannot read token file (fail closed)",
                        );
                        let mut data = serde_json::to_vec(&err_resp)?;
                        data.push(b'\n');
                        let _ = writer.write_all(&data).await;
                        let _ = writer.flush().await;
                        return Ok(());
                    }
                };

                if !constant_time_eq_token(&expected_token, &req.token) {
                    let err_resp = IpcResponse::err(req.id, "unauthorized", "invalid token");
                    let mut data = serde_json::to_vec(&err_resp)?;
                    data.push(b'\n');
                    let _ = writer.write_all(&data).await;
                    let _ = writer.flush().await;
                    return Ok(());
                }

                let resp = Self::dispatch_request(
                    req,
                    control.clone(),
                    bus.clone(),
                    routines.clone(),
                    directory.clone(),
                )
                .await;

                let mut out_data = serde_json::to_vec(&resp)?;
                out_data.push(b'\n');
                writer.write_all(&out_data).await?;
                writer.flush().await?;
                continue;
            }

            if unparsed.len() > MAX_LINE_BYTES {
                let err_resp =
                    IpcResponse::err(None, "invalid_params", "request line exceeds 1 MiB limit");
                let mut data = serde_json::to_vec(&err_resp)?;
                data.push(b'\n');
                let _ = writer.write_all(&data).await;
                let _ = writer.flush().await;
                return Ok(());
            }

            let n = reader.read(&mut buf).await?;
            if n == 0 {
                return Ok(());
            }
            unparsed.extend_from_slice(&buf[..n]);
        }
    }

    async fn dispatch_request(
        req: IpcRequest,
        control: Arc<C>,
        bus: Arc<Bus<D, S, A>>,
        routines: Arc<R>,
        directory: Arc<A>,
    ) -> IpcResponse {
        let req_id = req.id.clone();
        let agent_id = req.agent_id.as_deref();

        let caller_workspace = match agent_id {
            Some(aid) => {
                if let Some(agent) = directory.find_by_id(aid) {
                    Some(agent.workspace_id)
                } else {
                    return IpcResponse::err(
                        req_id,
                        "not_found",
                        &format!("calling agent {aid} not found"),
                    );
                }
            }
            None => None,
        };

        let result = match req.method.as_str() {
            "agents.list" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());
                match control.list_agents(ws).await {
                    Ok(agents) => Ok(serde_json::json!({ "agents": agents })),
                    Err(e) => Err(e),
                }
            }
            "agents.spawn" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref())
                    .unwrap_or("");
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let preset = req
                    .params
                    .get("preset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let role = req.params.get("role").and_then(|v| v.as_str());
                let floor = req.params.get("floor").and_then(|v| v.as_str());

                if name.is_empty() || preset.is_empty() {
                    Err(IpcError::InvalidParams(
                        "name and preset are required".into(),
                    ))
                } else {
                    match control.spawn_agent(ws, name, preset, role, floor).await {
                        Ok(agent) => Ok(serde_json::json!({ "agent": agent })),
                        Err(e) => Err(e),
                    }
                }
            }
            "agents.stop" => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if name.is_empty() {
                    Err(IpcError::InvalidParams("name is required".into()))
                } else {
                    match control.stop_agent(name, caller_workspace.as_deref()).await {
                        Ok(()) => Ok(serde_json::json!({})),
                        Err(e) => Err(e),
                    }
                }
            }
            "msg.ask" => {
                let to = req.params.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let body = req
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let timeout_s = req.params.get("timeout_s").and_then(|v| v.as_u64());
                let timeout = timeout_s.map(Duration::from_secs);

                if to.is_empty() || body.is_empty() {
                    Err(IpcError::InvalidParams("to and body are required".into()))
                } else {
                    match bus
                        .ask(agent_id, to, body, caller_workspace.as_deref(), timeout)
                        .await
                    {
                        Ok(reply) => Ok(serde_json::json!({
                            "reply": {
                                "id": reply.id,
                                "from": reply.from_agent,
                                "body": reply.body,
                            }
                        })),
                        Err(e) => Err(e.into()),
                    }
                }
            }
            "msg.notify" => {
                let to = req.params.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let body = req
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if to.is_empty() || body.is_empty() {
                    Err(IpcError::InvalidParams("to and body are required".into()))
                } else {
                    match bus
                        .notify(agent_id, to, body, caller_workspace.as_deref())
                        .await
                    {
                        Ok(msg_id) => Ok(serde_json::json!({ "message_id": msg_id })),
                        Err(e) => Err(e.into()),
                    }
                }
            }
            "msg.broadcast" => {
                let body = req
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref())
                    .unwrap_or("");

                if body.is_empty() {
                    Err(IpcError::InvalidParams("body is required".into()))
                } else {
                    match bus.broadcast(agent_id, ws, body).await {
                        Ok(ids) => Ok(serde_json::json!({ "message_ids": ids })),
                        Err(e) => Err(e.into()),
                    }
                }
            }
            "msg.reply" => {
                let message_id = req
                    .params
                    .get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let body = req
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if message_id.is_empty() || body.is_empty() {
                    Err(IpcError::InvalidParams(
                        "message_id and body are required".into(),
                    ))
                } else {
                    match bus.reply(message_id, agent_id, body).await {
                        Ok(_) => Ok(serde_json::json!({})),
                        Err(e) => Err(e.into()),
                    }
                }
            }
            "msg.inbox" => match agent_id {
                Some(aid) => match bus.inbox(aid).await {
                    Ok(msgs) => Ok(serde_json::json!({ "messages": msgs })),
                    Err(e) => Err(e.into()),
                },
                None => Err(IpcError::InvalidParams(
                    "msg.inbox requires an agent context (agent_id)".into(),
                )),
            },
            "notes.get" => {
                let title = req
                    .params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match control.get_note(title, caller_workspace.as_deref()).await {
                    Ok(note) => Ok(serde_json::to_value(note).unwrap()),
                    Err(e) => Err(e),
                }
            }
            "notes.set" => {
                let title = req
                    .params
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let body = req
                    .params
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                match control
                    .set_note(title, body, caller_workspace.as_deref())
                    .await
                {
                    Ok(note) => Ok(serde_json::to_value(note).unwrap()),
                    Err(e) => Err(e),
                }
            }
            "notes.list" => match control.list_notes(caller_workspace.as_deref()).await {
                Ok(notes) => Ok(serde_json::json!({ "notes": notes })),
                Err(e) => Err(e),
            },
            "floors.create" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref())
                    .unwrap_or("");
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let base_ref = req.params.get("base_ref").and_then(|v| v.as_str());
                match control.create_floor(ws, name, base_ref).await {
                    Ok(floor) => Ok(serde_json::to_value(floor).unwrap()),
                    Err(e) => Err(e),
                }
            }
            "floors.list" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());
                match control.list_floors(ws).await {
                    Ok(floors) => Ok(serde_json::json!({ "floors": floors })),
                    Err(e) => Err(e),
                }
            }
            "floors.remove" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref())
                    .unwrap_or("");
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let force = req
                    .params
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                match control.remove_floor(ws, name, force).await {
                    Ok(()) => Ok(serde_json::json!({})),
                    Err(e) => Err(e),
                }
            }
            "floors.merge" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref())
                    .unwrap_or("");
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let into = req
                    .params
                    .get("into")
                    .and_then(|v| v.as_str())
                    .unwrap_or("develop");
                match control.merge_floor(ws, name, into).await {
                    Ok(()) => Ok(serde_json::json!({})),
                    Err(e) => Err(e),
                }
            }
            "routines.list" => {
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());
                match routines.list_routines(ws).await {
                    Ok(r) => Ok(serde_json::json!({ "routines": r })),
                    Err(e) => Err(e),
                }
            }
            "routines.add" => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let schedule = req
                    .params
                    .get("schedule")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let target = req.params.get("target").and_then(|v| v.as_str());
                let command = req.params.get("command").and_then(|v| v.as_str());
                let body = req.params.get("body").and_then(|v| v.as_str());
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());

                match routines
                    .add_routine(name, schedule, target, command, body, ws)
                    .await
                {
                    Ok(r) => Ok(serde_json::to_value(r).unwrap()),
                    Err(e) => Err(e),
                }
            }
            "routines.remove" => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());
                match routines.remove_routine(name, ws).await {
                    Ok(()) => Ok(serde_json::json!({})),
                    Err(e) => Err(e),
                }
            }
            "routines.run" => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let ws = req
                    .params
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .or(caller_workspace.as_deref());
                match routines.run_routine(name, ws).await {
                    Ok(()) => Ok(serde_json::json!({})),
                    Err(e) => Err(e),
                }
            }
            "status" => match control.status().await {
                Ok(status) => Ok(serde_json::to_value(status).unwrap()),
                Err(e) => Err(e),
            },
            unknown => Err(IpcError::NotFound(format!("unknown method {unknown}"))),
        };

        match result {
            Ok(val) => IpcResponse::ok(req_id, val),
            Err(e) => IpcResponse::err(req_id, e.code(), &e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{InMemoryAgentDirectory, InMemoryDeliverer, InMemoryMessageStore};
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, BufReader};

    struct DummyControl;

    impl Control for DummyControl {
        async fn spawn_agent(
            &self,
            _ws: &str,
            _name: &str,
            _preset: &str,
            _role: Option<&str>,
            _floor: Option<&str>,
        ) -> Result<AgentSummary, IpcError> {
            Err(IpcError::Internal("dummy".into()))
        }

        async fn stop_agent(&self, _name: &str, _ws: Option<&str>) -> Result<(), IpcError> {
            Ok(())
        }

        async fn list_agents(&self, _ws: Option<&str>) -> Result<Vec<AgentSummary>, IpcError> {
            Ok(vec![])
        }

        async fn get_note(&self, title: &str, _ws: Option<&str>) -> Result<NoteRef, IpcError> {
            Ok(NoteRef {
                id: "n1".into(),
                workspace_id: "ws1".into(),
                title: title.into(),
                body: "dummy note".into(),
                updated_at: 0,
            })
        }

        async fn set_note(
            &self,
            title: &str,
            body: &str,
            _ws: Option<&str>,
        ) -> Result<NoteRef, IpcError> {
            Ok(NoteRef {
                id: "n1".into(),
                workspace_id: "ws1".into(),
                title: title.into(),
                body: body.into(),
                updated_at: 0,
            })
        }

        async fn list_notes(&self, _ws: Option<&str>) -> Result<Vec<NoteRef>, IpcError> {
            Ok(vec![])
        }

        async fn create_floor(
            &self,
            _ws: &str,
            _name: &str,
            _base: Option<&str>,
        ) -> Result<FloorRef, IpcError> {
            Err(IpcError::Internal("dummy".into()))
        }

        async fn list_floors(&self, _ws: Option<&str>) -> Result<Vec<FloorRef>, IpcError> {
            Ok(vec![])
        }

        async fn remove_floor(&self, _ws: &str, _name: &str, _force: bool) -> Result<(), IpcError> {
            Ok(())
        }

        async fn merge_floor(&self, _ws: &str, _name: &str, _into: &str) -> Result<(), IpcError> {
            Ok(())
        }

        async fn status(&self) -> Result<AppStatus, IpcError> {
            Ok(AppStatus {
                version: "0.1.0".into(),
                workspaces: vec!["default".into()],
                agents_running: 1,
                queued_messages: 0,
            })
        }
    }

    struct DummyRoutines;

    impl RoutineControl for DummyRoutines {
        async fn list_routines(&self, _ws: Option<&str>) -> Result<Vec<RoutineRef>, IpcError> {
            Ok(vec![])
        }

        async fn add_routine(
            &self,
            name: &str,
            schedule: &str,
            _target: Option<&str>,
            _command: Option<&str>,
            body: Option<&str>,
            _ws: Option<&str>,
        ) -> Result<RoutineRef, IpcError> {
            Ok(RoutineRef {
                id: "r1".into(),
                workspace_id: "ws1".into(),
                name: name.into(),
                schedule: schedule.into(),
                target_agent: None,
                command: None,
                body: body.unwrap_or("").into(),
                enabled: true,
                next_run_at: None,
            })
        }

        async fn remove_routine(&self, _name: &str, _ws: Option<&str>) -> Result<(), IpcError> {
            Ok(())
        }

        async fn run_routine(&self, _name: &str, _ws: Option<&str>) -> Result<(), IpcError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_ipc_bind_loopback_and_token_auth() {
        let dir = tempdir().unwrap();
        let token = config::read_or_create_token(dir.path()).unwrap();

        let deliverer = Arc::new(InMemoryDeliverer::default());
        let store = Arc::new(InMemoryMessageStore::default());
        let directory = Arc::new(InMemoryAgentDirectory::default());
        let bus = Arc::new(Bus::new(deliverer, store, directory.clone()));

        let server = IpcServer::new(
            dir.path().to_path_buf(),
            Arc::new(DummyControl),
            bus,
            Arc::new(DummyRoutines),
            directory,
        );

        let (addr, handle) = server.start().await.unwrap();
        assert!(addr.ip().is_loopback());

        let mut stream = TcpStream::connect(addr).await.unwrap();
        let req = serde_json::json!({
            "id": "1",
            "token": "wrong_token",
            "agent_id": null,
            "method": "status",
            "params": {}
        });

        let mut req_str = serde_json::to_string(&req).unwrap();
        req_str.push('\n');
        stream.write_all(req_str.as_bytes()).await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut resp_str = String::new();
        reader.read_line(&mut resp_str).await.unwrap();

        let resp: IpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, "unauthorized");

        // Connection should be closed after unauthorized
        let mut second_line = String::new();
        let n = reader.read_line(&mut second_line).await.unwrap();
        assert_eq!(n, 0);

        // Valid token connects and receives response
        let mut stream2 = TcpStream::connect(addr).await.unwrap();
        let req2 = serde_json::json!({
            "id": "2",
            "token": token,
            "agent_id": null,
            "method": "status",
            "params": {}
        });
        let mut req2_str = serde_json::to_string(&req2).unwrap();
        req2_str.push('\n');
        stream2.write_all(req2_str.as_bytes()).await.unwrap();

        let mut reader2 = BufReader::new(stream2);
        let mut resp2_str = String::new();
        reader2.read_line(&mut resp2_str).await.unwrap();

        let resp2: IpcResponse = serde_json::from_str(&resp2_str).unwrap();
        assert!(resp2.ok);
        assert_eq!(resp2.result.unwrap()["version"], "0.1.0");

        server.shutdown();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn test_ipc_fail_closed_without_token_file() {
        let dir = tempdir().unwrap();
        let deliverer = Arc::new(InMemoryDeliverer::default());
        let store = Arc::new(InMemoryMessageStore::default());
        let directory = Arc::new(InMemoryAgentDirectory::default());
        let bus = Arc::new(Bus::new(deliverer, store, directory.clone()));

        let server = IpcServer::new(
            dir.path().to_path_buf(),
            Arc::new(DummyControl),
            bus,
            Arc::new(DummyRoutines),
            directory,
        );

        let (addr, handle) = server.start().await.unwrap();

        let mut stream = TcpStream::connect(addr).await.unwrap();
        let req = serde_json::json!({
            "id": "1",
            "token": "any_token",
            "agent_id": null,
            "method": "status",
            "params": {}
        });

        let mut req_str = serde_json::to_string(&req).unwrap();
        req_str.push('\n');
        stream.write_all(req_str.as_bytes()).await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut resp_str = String::new();
        reader.read_line(&mut resp_str).await.unwrap();

        let resp: IpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, "unauthorized");

        server.shutdown();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn test_ipc_line_exceeds_1mib_limit() {
        let dir = tempdir().unwrap();
        let _token = config::read_or_create_token(dir.path()).unwrap();

        let deliverer = Arc::new(InMemoryDeliverer::default());
        let store = Arc::new(InMemoryMessageStore::default());
        let directory = Arc::new(InMemoryAgentDirectory::default());
        let bus = Arc::new(Bus::new(deliverer, store, directory.clone()));

        let server = IpcServer::new(
            dir.path().to_path_buf(),
            Arc::new(DummyControl),
            bus,
            Arc::new(DummyRoutines),
            directory,
        );

        let (addr, handle) = server.start().await.unwrap();

        let mut stream = TcpStream::connect(addr).await.unwrap();

        // Write 1 MiB + 100 bytes without newline
        let big_payload = vec![b'x'; MAX_LINE_BYTES + 100];
        stream.write_all(&big_payload).await.unwrap();

        let mut reader = BufReader::new(stream);
        let mut resp_str = String::new();
        reader.read_line(&mut resp_str).await.unwrap();

        let resp: IpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error.unwrap().code, "invalid_params");

        server.shutdown();
        let _ = handle.await;
    }
}
