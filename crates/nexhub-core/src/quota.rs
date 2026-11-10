//! Output stream watcher detecting provider quota limits and reset timers.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::bus::Deliverer;
use crate::model::{AgentRef, AgentState};

pub const DEFAULT_QUOTA_RETRY_MINUTES: i64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    #[error("regex compilation error: {0}")]
    Regex(#[from] regex::Error),
    #[error("agent not found: {0}")]
    AgentNotFound(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("delivery error: {0}")]
    Delivery(String),
}

#[derive(Debug, Clone)]
pub struct QuotaPresetConfig {
    pub quota_patterns: Vec<Regex>,
    pub quota_reset_regex: Option<Regex>,
}

impl QuotaPresetConfig {
    pub fn new(patterns: &[&str], reset_regex: Option<&str>) -> Result<Self, QuotaError> {
        let mut quota_patterns = Vec::new();
        for p in patterns {
            quota_patterns.push(Regex::new(p)?);
        }
        let quota_reset_regex = match reset_regex {
            Some(r) => Some(Regex::new(r)?),
            None => None,
        };
        Ok(Self {
            quota_patterns,
            quota_reset_regex,
        })
    }

    pub fn matches(&self, text: &str) -> bool {
        self.quota_patterns.iter().any(|re| re.is_match(text))
    }

    pub fn parse_reset_time(&self, text: &str, from_now: DateTime<Utc>) -> DateTime<Utc> {
        if let Some(ref re) = self.quota_reset_regex {
            if let Some(caps) = re.captures(text) {
                if let Some(at_match) = caps.name("at") {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(at_match.as_str()) {
                        return dt.with_timezone(&Utc);
                    }
                }
                if let Some(min_match) = caps.name("in_minutes") {
                    if let Ok(mins) = min_match.as_str().parse::<i64>() {
                        return from_now + chrono::Duration::minutes(mins);
                    }
                }
            }
        }
        from_now + chrono::Duration::minutes(DEFAULT_QUOTA_RETRY_MINUTES)
    }
}

pub fn format_resume_message(last_prompt: Option<&str>) -> String {
    match last_prompt {
        Some(prompt) => {
            let summary = prompt
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("");
            if summary.is_empty() {
                "continue exactly where you stopped".to_string()
            } else {
                format!("continue exactly where you stopped\n{summary}")
            }
        }
        None => "continue exactly where you stopped".to_string(),
    }
}

pub trait QuotaAgentStore: Send + Sync {
    fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRef>, QuotaError>;
    fn list_waiting_quota(&self) -> Result<Vec<AgentRef>, QuotaError>;
    fn update_agent_state(
        &self,
        agent_id: &str,
        state: AgentState,
        reset_at: Option<i64>,
    ) -> Result<(), QuotaError>;
    fn log_event(&self, kind: &str, data_json: &str) -> Result<(), QuotaError>;
}

pub struct QuotaSupervisor<S, D>
where
    S: QuotaAgentStore,
    D: Deliverer,
{
    store: Arc<S>,
    deliverer: Arc<D>,
}

impl<S, D> QuotaSupervisor<S, D>
where
    S: QuotaAgentStore,
    D: Deliverer,
{
    pub fn new(store: Arc<S>, deliverer: Arc<D>) -> Self {
        Self { store, deliverer }
    }

    /// Process output stream from an agent. If quota pattern matches, transition to `waiting_quota`.
    pub fn on_agent_output(
        &self,
        agent_id: &str,
        output_chunk: &str,
        config: &QuotaPresetConfig,
    ) -> Result<bool, QuotaError> {
        if config.matches(output_chunk) {
            let now = Utc::now();
            let reset_time = config.parse_reset_time(output_chunk, now);
            let reset_at_ms = reset_time.timestamp_millis();

            self.store
                .update_agent_state(agent_id, AgentState::WaitingQuota, Some(reset_at_ms))?;

            let event_data = serde_json::json!({
                "agent_id": agent_id,
                "quota_reset_at": reset_at_ms,
            });
            let _ = self
                .store
                .log_event("agent_waiting_quota", &event_data.to_string());

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Periodically check agents in `waiting_quota`. If reset time reached, resume them.
    pub fn tick(&self) -> Result<usize, QuotaError> {
        let now = Utc::now();
        let now_ms = now.timestamp_millis();
        let waiting = self.store.list_waiting_quota()?;
        let mut resumed_count = 0;

        for agent in waiting {
            if let Some(reset_ms) = agent.quota_reset_at {
                if reset_ms <= now_ms {
                    let resume_msg = format_resume_message(agent.last_prompt.as_deref());
                    self.deliverer
                        .deliver(&agent.id, &resume_msg)
                        .map_err(|e| QuotaError::Delivery(e.to_string()))?;

                    self.store
                        .update_agent_state(&agent.id, AgentState::Busy, None)?;

                    let event_data = serde_json::json!({
                        "agent_id": agent.id,
                        "action": "resume_from_quota",
                    });
                    let _ = self
                        .store
                        .log_event("agent_resumed_quota", &event_data.to_string());

                    resumed_count += 1;
                }
            }
        }

        Ok(resumed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusError;
    use std::sync::Mutex;

    #[test]
    fn test_agy_pattern_triggers_waiting_quota() {
        let agy_patterns = vec!["Waiting for quota reset", "(?i)quota exceeded"];
        let config = QuotaPresetConfig::new(&agy_patterns, None).unwrap();

        assert!(config.matches("Error: Waiting for quota reset"));
        assert!(config.matches("Model quota exceeded. Please try again."));
        assert!(!config.matches("Compiling packages..."));
    }

    #[test]
    fn test_claude_pattern_triggers_waiting_quota() {
        let claude_patterns = vec!["(?i)usage limit"];
        let config = QuotaPresetConfig::new(&claude_patterns, None).unwrap();

        assert!(config.matches("You have reached your usage limit."));
        assert!(!config.matches("Running cargo check"));
    }

    #[test]
    fn test_reset_in_minutes_parsing() {
        let reset_pattern = r"(?i)reset in (?P<in_minutes>\d+) minutes";
        let config = QuotaPresetConfig::new(&["quota exceeded"], Some(reset_pattern)).unwrap();

        let output = "quota exceeded, reset in 15 minutes";
        assert!(config.matches(output));

        let now = Utc::now();
        let reset = config.parse_reset_time(output, now);
        let diff = reset - now;
        assert_eq!(diff.num_minutes(), 15);
    }

    #[test]
    fn test_reset_at_rfc3339_parsing() {
        let reset_pattern = r"(?i)reset at (?P<at>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)";
        let config = QuotaPresetConfig::new(&["quota exceeded"], Some(reset_pattern)).unwrap();

        let output = "quota exceeded: reset at 2026-11-10T15:00:00Z";
        assert!(config.matches(output));

        let now = Utc::now();
        let reset = config.parse_reset_time(output, now);
        assert_eq!(reset.to_rfc3339(), "2026-11-10T15:00:00+00:00");
    }

    #[derive(Default)]
    struct MockQuotaStore {
        agents: Mutex<Vec<AgentRef>>,
        events: Mutex<Vec<(String, String)>>,
    }

    impl QuotaAgentStore for MockQuotaStore {
        fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRef>, QuotaError> {
            let agents = self.agents.lock().unwrap();
            Ok(agents.iter().find(|a| a.id == agent_id).cloned())
        }

        fn list_waiting_quota(&self) -> Result<Vec<AgentRef>, QuotaError> {
            let agents = self.agents.lock().unwrap();
            Ok(agents
                .iter()
                .filter(|a| a.state == AgentState::WaitingQuota)
                .cloned()
                .collect())
        }

        fn update_agent_state(
            &self,
            agent_id: &str,
            state: AgentState,
            reset_at: Option<i64>,
        ) -> Result<(), QuotaError> {
            let mut agents = self.agents.lock().unwrap();
            if let Some(agent) = agents.iter_mut().find(|a| a.id == agent_id) {
                agent.state = state;
                agent.quota_reset_at = reset_at;
            }
            Ok(())
        }

        fn log_event(&self, kind: &str, data_json: &str) -> Result<(), QuotaError> {
            let mut events = self.events.lock().unwrap();
            events.push((kind.to_string(), data_json.to_string()));
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockDeliverer {
        deliveries: Mutex<Vec<(String, String)>>,
    }

    impl Deliverer for MockDeliverer {
        fn deliver(&self, agent_id: &str, text: &str) -> Result<(), BusError> {
            self.deliveries
                .lock()
                .unwrap()
                .push((agent_id.to_string(), text.to_string()));
            Ok(())
        }
    }

    #[test]
    fn test_quota_supervisor_workflow() {
        let store = Arc::new(MockQuotaStore::default());
        let deliverer = Arc::new(MockDeliverer::default());

        let agent = AgentRef {
            id: "agent-1".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "builder".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: AgentState::Running,
            quota_reset_at: None,
            last_prompt: Some("implement feature X\nmore lines".to_string()),
            created_at: 0,
        };
        store.agents.lock().unwrap().push(agent);

        let config = QuotaPresetConfig::new(
            &["Waiting for quota reset"],
            Some(r"reset in (?P<in_minutes>\d+) minutes"),
        )
        .unwrap();

        let supervisor = QuotaSupervisor::new(store.clone(), deliverer.clone());

        // Agent outputs quota message
        let matched = supervisor
            .on_agent_output(
                "agent-1",
                "Waiting for quota reset, reset in 10 minutes",
                &config,
            )
            .unwrap();
        assert!(matched);

        // Agent should now be WaitingQuota
        {
            let agents = store.agents.lock().unwrap();
            assert_eq!(agents[0].state, AgentState::WaitingQuota);
            assert!(agents[0].quota_reset_at.is_some());
        }

        // Tick before reset time should do nothing
        assert_eq!(supervisor.tick().unwrap(), 0);
        assert_eq!(deliverer.deliveries.lock().unwrap().len(), 0);

        // Manually move reset time to past
        {
            let mut agents = store.agents.lock().unwrap();
            agents[0].quota_reset_at = Some(Utc::now().timestamp_millis() - 1000);
        }

        // Tick should resume agent
        let resumed = supervisor.tick().unwrap();
        assert_eq!(resumed, 1);

        {
            let agents = store.agents.lock().unwrap();
            assert_eq!(agents[0].state, AgentState::Busy);
        }

        let d = deliverer.deliveries.lock().unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "agent-1");
        assert_eq!(
            d[0].1,
            "continue exactly where you stopped\nimplement feature X"
        );
    }
}
