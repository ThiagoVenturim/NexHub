//! Session manager tracking active PTY sessions and agent lifecycles.

use crate::db::Db;
use crate::model::{AgentState, SpawnSpec};
use crate::pty::{PtyError, PtySession};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("pty error: {0}")]
    Pty(#[from] PtyError),

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("db error: {0}")]
    Db(String),
}

/// Trait defining idle detection heuristics for an agent session.
pub trait IdleDetector: Send + Sync {
    /// Called when a prompt is delivered to the agent, transitioning to busy.
    fn on_prompt_delivered(&self);

    /// Called when output bytes arrive from the agent's PTY.
    fn on_output(&self, bytes: &[u8]);

    /// Returns true if the agent is idle.
    fn is_idle(&self) -> bool;

    /// Returns true if the agent is busy.
    fn is_busy(&self) -> bool {
        !self.is_idle()
    }
}

/// Time-based idle detector: transitions to busy on delivery, stays busy while output arrives,
/// and returns to idle when `idle_duration` elapses without any new output.
#[derive(Debug)]
pub struct TimeIdleDetector {
    idle_duration: Duration,
    state: Mutex<TimeIdleState>,
}

#[derive(Debug)]
struct TimeIdleState {
    busy: bool,
    last_activity: Option<Instant>,
}

impl TimeIdleDetector {
    pub fn new(idle_duration: Duration) -> Self {
        Self {
            idle_duration,
            state: Mutex::new(TimeIdleState {
                busy: false,
                last_activity: None,
            }),
        }
    }

    pub fn from_millis(ms: u64) -> Self {
        Self::new(Duration::from_millis(ms))
    }

    /// Asynchronously polls until the detector reports idle.
    pub async fn wait_for_idle(&self) {
        while !self.is_idle() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl IdleDetector for TimeIdleDetector {
    fn on_prompt_delivered(&self) {
        let mut state = self.state.lock().unwrap();
        state.busy = true;
        state.last_activity = Some(Instant::now());
    }

    fn on_output(&self, _bytes: &[u8]) {
        let mut state = self.state.lock().unwrap();
        if state.busy {
            state.last_activity = Some(Instant::now());
        }
    }

    fn is_idle(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.busy {
            return true;
        }
        if let Some(last) = state.last_activity {
            if last.elapsed() >= self.idle_duration {
                state.busy = false;
                state.last_activity = None;
                return true;
            }
        }
        false
    }
}

struct SessionEntry {
    session: PtySession,
    detector: Arc<dyn IdleDetector>,
}

/// Coordinates active PTY sessions, agent states, and output streams.
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, SessionEntry>>>,
    db: Option<Arc<Db>>,
}

impl SessionManager {
    pub fn new(db: Option<Arc<Db>>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            db,
        }
    }

    /// Spawns an agent process with the given specification and idle timeout.
    pub fn spawn_agent(
        &self,
        agent_id: &str,
        spec: SpawnSpec,
        idle_ms: u64,
    ) -> Result<(), SessionError> {
        let session = PtySession::spawn(spec)?;
        let detector = Arc::new(TimeIdleDetector::from_millis(idle_ms));

        if let Some(db) = &self.db {
            let _ = db.set_agent_state(agent_id, AgentState::Running);
        }

        let mut lock = self.sessions.lock().unwrap();
        lock.insert(agent_id.to_string(), SessionEntry { session, detector });
        Ok(())
    }

    /// Writes raw bytes to an active agent session.
    pub fn write(&self, agent_id: &str, bytes: &[u8]) -> Result<(), SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        entry.session.write(bytes)?;
        entry.detector.on_output(bytes);
        Ok(())
    }

    /// Delivers a prompt text with trailing carriage return (and bracketed paste if requested)
    /// and marks the agent as busy.
    pub fn deliver_prompt(
        &self,
        agent_id: &str,
        text: &str,
        bracketed_paste: bool,
    ) -> Result<(), SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        entry.session.deliver_prompt(text, bracketed_paste)?;
        entry.detector.on_prompt_delivered();

        if let Some(db) = &self.db {
            let _ = db.set_agent_state(agent_id, AgentState::Busy);
        }
        Ok(())
    }

    /// Resizes the PTY dimensions for an active agent.
    pub fn resize(&self, agent_id: &str, cols: u16, rows: u16) -> Result<(), SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        entry.session.resize(cols, rows)?;
        Ok(())
    }

    /// Terminates an agent's process.
    pub fn kill(&self, agent_id: &str) -> Result<(), SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        entry.session.kill()?;

        if let Some(db) = &self.db {
            let _ = db.set_agent_state(agent_id, AgentState::Stopped);
        }
        Ok(())
    }

    /// Subscribes to realtime output chunks for an agent.
    pub fn subscribe(&self, agent_id: &str) -> Result<broadcast::Receiver<Vec<u8>>, SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        Ok(entry.session.subscribe())
    }

    /// Returns the recent output history stored in the ring buffer.
    pub fn recent_output(&self, agent_id: &str) -> Result<Vec<u8>, SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        Ok(entry.session.recent_output())
    }

    /// Checks whether an agent is currently considered idle.
    pub fn is_idle(&self, agent_id: &str) -> Result<bool, SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        let idle = entry.detector.is_idle();
        if idle {
            if let Some(db) = &self.db {
                let _ = db.set_agent_state(agent_id, AgentState::Running);
            }
        }
        Ok(idle)
    }

    /// Checks if the agent process is still running.
    pub fn is_alive(&self, agent_id: &str) -> Result<bool, SessionError> {
        let lock = self.sessions.lock().unwrap();
        let entry = lock
            .get(agent_id)
            .ok_or_else(|| SessionError::AgentNotFound(agent_id.to_string()))?;
        Ok(entry.session.is_alive())
    }

    /// Removes an agent from tracking.
    pub fn remove(&self, agent_id: &str) -> Option<()> {
        let mut lock = self.sessions.lock().unwrap();
        lock.remove(agent_id).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_idle_detector_transitions() {
        let detector = TimeIdleDetector::from_millis(50);
        // Initially idle
        assert!(detector.is_idle());

        // Prompt delivered -> busy
        detector.on_prompt_delivered();
        assert!(!detector.is_idle());
        assert!(detector.is_busy());

        // Output arrived while busy -> resets timer, still busy
        detector.on_output(b"processing...\n");
        assert!(!detector.is_idle());

        // Wait for idle_ms without output
        detector.wait_for_idle().await;
        assert!(detector.is_idle());
        assert!(!detector.is_busy());
    }

    fn find_echo_agent_bin() -> PathBuf {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_echo_agent") {
            let p = PathBuf::from(path);
            if p.exists() {
                return p;
            }
        }

        if let Ok(curr) = std::env::current_exe() {
            if let Some(parent) = curr.parent() {
                let candidate = parent
                    .parent()
                    .unwrap_or(parent)
                    .join(format!("echo_agent{}", std::env::consts::EXE_SUFFIX));
                if candidate.exists() {
                    return candidate;
                }
            }
        }

        PathBuf::from(format!("echo_agent{}", std::env::consts::EXE_SUFFIX))
    }

    #[tokio::test]
    async fn test_pty_echo_agent_lifecycle() {
        let echo_bin = find_echo_agent_bin();
        let spec = SpawnSpec {
            program: echo_bin.to_string_lossy().to_string(),
            args: vec![],
            cwd: PathBuf::from("."),
            env: vec![],
            cols: 80,
            rows: 24,
        };

        let session = PtySession::spawn(spec).unwrap();
        let mut rx = session.subscribe();

        // On Windows ConPTY, answer DSR query if emitted
        let _ = session.write(b"\x1b[1;1R");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Write "oi\r\n" and "oi\r"
        session.write(b"oi\r\n").unwrap();
        session.write(b"oi\r").unwrap();

        // Read output until we see "echo: oi"
        let mut output_str = String::new();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Ok(bytes)) => {
                    output_str.push_str(&String::from_utf8_lossy(&bytes));
                    if output_str.contains("echo: oi") {
                        break;
                    }
                }
                _ => {}
            }
        }
        assert!(
            output_str.contains("echo: oi"),
            "expected output to contain 'echo: oi', got: {output_str}"
        );

        // Resize should not fail
        assert!(session.resize(100, 30).is_ok());

        // Kill terminates process
        assert!(session.kill().is_ok());

        // Recent output in ring buffer is retained after kill
        tokio::time::sleep(Duration::from_millis(100)).await;
        let history = session.recent_output();
        let history_str = String::from_utf8_lossy(&history);
        assert!(
            history_str.contains("echo: oi"),
            "ring buffer should contain 'echo: oi' after termination, got: {history_str}"
        );
    }
}
