//! Scheduled routines supervisor and execution runner.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Datelike, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use crate::bus::{AgentDirectory, Bus, BusError, Deliverer, MessageStore};
use crate::model::RoutineRef;

#[derive(Debug, thiserror::Error)]
pub enum RoutineError {
    #[error("invalid schedule format: {0}")]
    InvalidSchedule(String),
    #[error("command execution error: {0}")]
    Execution(String),
    #[error("failed to parse command: {0}")]
    CommandParse(String),
    #[error("routine store error: {0}")]
    Store(String),
    #[error("bus error: {0}")]
    Bus(#[from] BusError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schedule {
    EveryMinutes(u32),
    EveryHours(u32),
    Daily { hour: u32, minute: u32 },
    Weekdays { hour: u32, minute: u32 },
    Cron(String),
}

pub fn parse_schedule(input: &str) -> Result<Schedule, RoutineError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(RoutineError::InvalidSchedule("empty schedule".into()));
    }

    if let Some(rest) = s.strip_prefix("every ") {
        let rest = rest.trim();
        if let Some(mins) = rest.strip_suffix('m') {
            let m: u32 = mins
                .trim()
                .parse()
                .map_err(|_| RoutineError::InvalidSchedule(format!("invalid minutes in: {s}")))?;
            if m == 0 {
                return Err(RoutineError::InvalidSchedule("minutes must be > 0".into()));
            }
            return Ok(Schedule::EveryMinutes(m));
        } else if let Some(hours) = rest.strip_suffix('h') {
            let h: u32 = hours
                .trim()
                .parse()
                .map_err(|_| RoutineError::InvalidSchedule(format!("invalid hours in: {s}")))?;
            if h == 0 {
                return Err(RoutineError::InvalidSchedule("hours must be > 0".into()));
            }
            return Ok(Schedule::EveryHours(h));
        } else {
            return Err(RoutineError::InvalidSchedule(format!(
                "every schedule must end in 'm' or 'h': {s}"
            )));
        }
    }

    if let Some(rest) = s.strip_prefix("daily ") {
        let (hour, minute) = parse_time(rest.trim(), s)?;
        return Ok(Schedule::Daily { hour, minute });
    }

    if let Some(rest) = s.strip_prefix("weekdays ") {
        let (hour, minute) = parse_time(rest.trim(), s)?;
        return Ok(Schedule::Weekdays { hour, minute });
    }

    if let Some(rest) = s.strip_prefix("cron ") {
        let mut expr = rest.trim();
        if (expr.starts_with('"') && expr.ends_with('"'))
            || (expr.starts_with('\'') && expr.ends_with('\''))
        {
            expr = &expr[1..expr.len() - 1];
        }
        let cron_expr = expr.trim();
        // Standard 5-field cron: min hour dom month dow
        // Prepend 0 for seconds to validate with cron crate
        let cron_6 = format!("0 {cron_expr}");
        cron::Schedule::from_str(&cron_6).map_err(|e| {
            RoutineError::InvalidSchedule(format!("invalid cron expression '{cron_expr}': {e}"))
        })?;
        return Ok(Schedule::Cron(cron_expr.to_string()));
    }

    Err(RoutineError::InvalidSchedule(format!(
        "unrecognized schedule syntax: '{s}'"
    )))
}

fn parse_time(time_str: &str, full_schedule: &str) -> Result<(u32, u32), RoutineError> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return Err(RoutineError::InvalidSchedule(format!(
            "time must be in HH:MM format: {full_schedule}"
        )));
    }
    let hour: u32 = parts[0]
        .parse()
        .map_err(|_| RoutineError::InvalidSchedule(format!("invalid hour in: {full_schedule}")))?;
    let minute: u32 = parts[1].parse().map_err(|_| {
        RoutineError::InvalidSchedule(format!("invalid minute in: {full_schedule}"))
    })?;
    if hour > 23 || minute > 59 {
        return Err(RoutineError::InvalidSchedule(format!(
            "time out of range (00:00-23:59): {full_schedule}"
        )));
    }
    Ok((hour, minute))
}

pub fn calculate_next_run(
    schedule: &Schedule,
    from: DateTime<Utc>,
) -> Result<DateTime<Utc>, RoutineError> {
    match schedule {
        Schedule::EveryMinutes(m) => Ok(from + chrono::Duration::minutes(*m as i64)),
        Schedule::EveryHours(h) => Ok(from + chrono::Duration::hours(*h as i64)),
        Schedule::Daily { hour, minute } => {
            let target_time = NaiveTime::from_hms_opt(*hour, *minute, 0).unwrap();
            let today_target = from.date_naive().and_time(target_time).and_utc();

            if today_target > from {
                Ok(today_target)
            } else {
                let tomorrow = from.date_naive() + chrono::Days::new(1);
                Ok(tomorrow.and_time(target_time).and_utc())
            }
        }
        Schedule::Weekdays { hour, minute } => {
            let target_time = NaiveTime::from_hms_opt(*hour, *minute, 0).unwrap();
            for day_offset in 0..=7 {
                let candidate_date = from.date_naive() + chrono::Days::new(day_offset);
                let weekday = candidate_date.weekday().number_from_monday();
                if weekday <= 5 {
                    let candidate = candidate_date.and_time(target_time).and_utc();
                    if candidate > from {
                        return Ok(candidate);
                    }
                }
            }
            // Fallback: 1 week ahead
            let next_week = from.date_naive() + chrono::Days::new(7);
            Ok(next_week.and_time(target_time).and_utc())
        }
        Schedule::Cron(expr) => {
            let cron_6 = format!("0 {expr}");
            let sched = cron::Schedule::from_str(&cron_6).map_err(|e| {
                RoutineError::InvalidSchedule(format!("invalid cron expression '{expr}': {e}"))
            })?;
            sched
                .after(&from)
                .next()
                .ok_or_else(|| RoutineError::InvalidSchedule("no next run found for cron".into()))
        }
    }
}

pub fn next_run_at(schedule_str: &str, after_ms: i64) -> Result<i64, RoutineError> {
    let sched = parse_schedule(schedule_str)?;
    let from = DateTime::<Utc>::from_timestamp_millis(after_ms).unwrap_or_else(Utc::now);
    let next = calculate_next_run(&sched, from)?;
    Ok(next.timestamp_millis())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRunResult {
    pub exit_code: Option<i32>,
    pub output_tail: String,
}

pub async fn execute_command_no_shell(
    cmd_line: &str,
    working_dir: &Path,
) -> Result<CommandRunResult, RoutineError> {
    let parts = shell_words::split(cmd_line)
        .map_err(|e| RoutineError::CommandParse(format!("shell-words parse error: {e}")))?;

    if parts.is_empty() {
        return Err(RoutineError::CommandParse("empty command line".into()));
    }

    let program = &parts[0];
    let args = &parts[1..];

    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(working_dir)
        .output()
        .await
        .map_err(|e| RoutineError::Execution(format!("failed to execute '{program}': {e}")))?;

    let mut combined = Vec::new();
    combined.extend_from_slice(&output.stdout);
    if !output.stderr.is_empty() {
        if !combined.is_empty() && !combined.ends_with(b"\n") {
            combined.push(b'\n');
        }
        combined.extend_from_slice(&output.stderr);
    }

    const MAX_TAIL_BYTES: usize = 8192; // 8 KiB
    let tail_bytes = if combined.len() > MAX_TAIL_BYTES {
        &combined[combined.len() - MAX_TAIL_BYTES..]
    } else {
        &combined[..]
    };

    let output_tail = String::from_utf8_lossy(tail_bytes).into_owned();

    Ok(CommandRunResult {
        exit_code: output.status.code(),
        output_tail,
    })
}

pub trait RoutineStore: Send + Sync {
    fn list_enabled(&self) -> Result<Vec<RoutineRef>, RoutineError>;
    fn update_next_run(
        &self,
        routine_id: &str,
        next_run_at: Option<i64>,
    ) -> Result<(), RoutineError>;
    fn record_run(
        &self,
        routine_id: &str,
        started_at: i64,
        finished_at: i64,
        exit_code: Option<i32>,
        output_tail: Option<&str>,
    ) -> Result<String, RoutineError>;
    fn get_workspace_repo_path(&self, workspace_id: &str) -> Result<Option<String>, RoutineError>;
}

pub struct RoutineRunner<S, D, M, A>
where
    S: RoutineStore,
    D: Deliverer,
    M: MessageStore,
    A: AgentDirectory,
{
    store: Arc<S>,
    bus: Arc<Bus<D, M, A>>,
}

impl<S, D, M, A> RoutineRunner<S, D, M, A>
where
    S: RoutineStore,
    D: Deliverer,
    M: MessageStore,
    A: AgentDirectory,
{
    pub fn new(store: Arc<S>, bus: Arc<Bus<D, M, A>>) -> Self {
        Self { store, bus }
    }

    /// Missed runs while the app was closed: run once on startup, not once per missed slot.
    pub async fn run_missed_on_startup(&self) -> Result<usize, RoutineError> {
        let now = Utc::now();
        let now_ms = now.timestamp_millis();
        let enabled = self.store.list_enabled()?;
        let mut executed_count = 0;

        for routine in enabled {
            if let Some(next_ms) = routine.next_run_at {
                if next_ms <= now_ms {
                    self.execute_routine(&routine, now_ms).await?;
                    executed_count += 1;

                    // Reschedule for next slot after now (not once per missed slot)
                    if let Ok(sched) = parse_schedule(&routine.schedule) {
                        let next_run = calculate_next_run(&sched, now)?;
                        self.store
                            .update_next_run(&routine.id, Some(next_run.timestamp_millis()))?;
                    }
                }
            }
        }

        Ok(executed_count)
    }

    /// Single scheduler tick (runs every 15s in background).
    pub async fn tick(&self) -> Result<usize, RoutineError> {
        let now = Utc::now();
        let now_ms = now.timestamp_millis();
        let enabled = self.store.list_enabled()?;
        let mut executed = 0;

        for routine in enabled {
            if let Some(next_ms) = routine.next_run_at {
                if next_ms <= now_ms {
                    self.execute_routine(&routine, now_ms).await?;
                    executed += 1;

                    if let Ok(sched) = parse_schedule(&routine.schedule) {
                        let next_run = calculate_next_run(&sched, now)?;
                        self.store
                            .update_next_run(&routine.id, Some(next_run.timestamp_millis()))?;
                    }
                }
            }
        }

        Ok(executed)
    }

    pub async fn execute_routine(
        &self,
        routine: &RoutineRef,
        started_at: i64,
    ) -> Result<(), RoutineError> {
        if let Some(ref target_agent) = routine.target_agent {
            // Deliver prompt via bus as notify from "routine:<name>"
            let sender = format!("routine:{}", routine.name);
            let _ = self
                .bus
                .notify(
                    Some(&sender),
                    target_agent,
                    &routine.body,
                    Some(&routine.workspace_id),
                )
                .await?;

            let finished_at = Utc::now().timestamp_millis();
            self.store
                .record_run(&routine.id, started_at, finished_at, Some(0), None)?;
        } else if let Some(ref cmd) = routine.command {
            let repo_path = self
                .store
                .get_workspace_repo_path(&routine.workspace_id)?
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));

            let res = execute_command_no_shell(cmd, &repo_path).await?;
            let finished_at = Utc::now().timestamp_millis();
            self.store.record_run(
                &routine.id,
                started_at,
                finished_at,
                res.exit_code,
                Some(&res.output_tail),
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::sync::Mutex;

    #[test]
    fn test_grammar_every_minutes() {
        let s = parse_schedule("every 15m").unwrap();
        assert_eq!(s, Schedule::EveryMinutes(15));

        let s2 = parse_schedule("every 45m").unwrap();
        assert_eq!(s2, Schedule::EveryMinutes(45));
    }

    #[test]
    fn test_grammar_every_hours() {
        let s = parse_schedule("every 2h").unwrap();
        assert_eq!(s, Schedule::EveryHours(2));

        let s2 = parse_schedule("every 1h").unwrap();
        assert_eq!(s2, Schedule::EveryHours(1));
    }

    #[test]
    fn test_grammar_daily() {
        let s = parse_schedule("daily 08:00").unwrap();
        assert_eq!(s, Schedule::Daily { hour: 8, minute: 0 });

        let s2 = parse_schedule("daily 23:30").unwrap();
        assert_eq!(
            s2,
            Schedule::Daily {
                hour: 23,
                minute: 30
            }
        );
    }

    #[test]
    fn test_grammar_weekdays() {
        let s = parse_schedule("weekdays 09:30").unwrap();
        assert_eq!(
            s,
            Schedule::Weekdays {
                hour: 9,
                minute: 30
            }
        );
    }

    #[test]
    fn test_grammar_cron() {
        let s = parse_schedule("cron \"*/5 * * * *\"").unwrap();
        assert_eq!(s, Schedule::Cron("*/5 * * * *".to_string()));

        let s2 = parse_schedule("cron 0 12 * * 1-5").unwrap();
        assert_eq!(s2, Schedule::Cron("0 12 * * 1-5".to_string()));
    }

    #[test]
    fn test_daily_crossing_midnight() {
        let sched = Schedule::Daily { hour: 0, minute: 5 };

        // Before midnight: 23:55 on 2026-11-09
        let from = Utc.with_ymd_and_hms(2026, 11, 9, 23, 55, 0).unwrap();
        let next = calculate_next_run(&sched, from).unwrap();
        // Should cross midnight and run at 00:05 on 2026-11-10
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 11, 10, 0, 5, 0).unwrap());

        // After midnight: 00:06 on 2026-11-10
        let from_after = Utc.with_ymd_and_hms(2026, 11, 10, 0, 6, 0).unwrap();
        let next_after = calculate_next_run(&sched, from_after).unwrap();
        // Should run at 00:05 on 2026-11-11
        assert_eq!(
            next_after,
            Utc.with_ymd_and_hms(2026, 11, 11, 0, 5, 0).unwrap()
        );
    }

    #[tokio::test]
    async fn test_command_quotes_split_and_no_shell() {
        // Test shell-words parsing without invoking a shell
        let cmd = "echo \"hello world\" 'single quoted' arg3";
        let parts = shell_words::split(cmd).unwrap();
        assert_eq!(parts, vec!["echo", "hello world", "single quoted", "arg3"]);

        // Execute a direct command without shell
        #[cfg(windows)]
        let cmd_line = "cmd.exe /c echo hello";
        #[cfg(not(windows))]
        let cmd_line = "echo hello";

        let current_dir = std::env::current_dir().unwrap();
        let res = execute_command_no_shell(cmd_line, &current_dir)
            .await
            .unwrap();
        assert_eq!(res.exit_code, Some(0));
        assert!(res.output_tail.contains("hello"));
    }

    #[derive(Default)]
    struct MockRoutineStore {
        routines: Mutex<Vec<RoutineRef>>,
        runs: Mutex<Vec<(String, i64, i64, Option<i32>, Option<String>)>>,
    }

    impl RoutineStore for MockRoutineStore {
        fn list_enabled(&self) -> Result<Vec<RoutineRef>, RoutineError> {
            let r = self.routines.lock().unwrap();
            Ok(r.iter().filter(|x| x.enabled).cloned().collect())
        }

        fn update_next_run(
            &self,
            routine_id: &str,
            next_run_at: Option<i64>,
        ) -> Result<(), RoutineError> {
            let mut r = self.routines.lock().unwrap();
            if let Some(item) = r.iter_mut().find(|x| x.id == routine_id) {
                item.next_run_at = next_run_at;
            }
            Ok(())
        }

        fn record_run(
            &self,
            routine_id: &str,
            started_at: i64,
            finished_at: i64,
            exit_code: Option<i32>,
            output_tail: Option<&str>,
        ) -> Result<String, RoutineError> {
            let mut runs = self.runs.lock().unwrap();
            runs.push((
                routine_id.to_string(),
                started_at,
                finished_at,
                exit_code,
                output_tail.map(|s| s.to_string()),
            ));
            Ok("run-1".to_string())
        }

        fn get_workspace_repo_path(
            &self,
            _workspace_id: &str,
        ) -> Result<Option<String>, RoutineError> {
            Ok(Some(".".to_string()))
        }
    }

    #[tokio::test]
    async fn test_missed_runs_policy_runs_once_on_startup() {
        use crate::bus::{InMemoryAgentDirectory, InMemoryDeliverer, InMemoryMessageStore};
        use crate::model::AgentRef;

        let store = Arc::new(MockRoutineStore::default());
        let deliverer = Arc::new(InMemoryDeliverer::default());
        let msg_store = Arc::new(InMemoryMessageStore::default());
        let directory = Arc::new(InMemoryAgentDirectory::default());

        directory.add(AgentRef {
            id: "agent-1".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: None,
            name: "worker".to_string(),
            preset: "agy".to_string(),
            role: None,
            cwd: "/".to_string(),
            state: crate::model::AgentState::Running,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 0,
        });

        let bus = Arc::new(Bus::new(deliverer, msg_store, directory));
        let runner = RoutineRunner::new(store.clone(), bus);

        // Schedule was due 2 hours ago (multiple slots missed for every 15m)
        let two_hours_ago = Utc::now() - chrono::Duration::hours(2);
        store.routines.lock().unwrap().push(RoutineRef {
            id: "r1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "standup".to_string(),
            schedule: "every 15m".to_string(),
            target_agent: Some("worker".to_string()),
            command: None,
            body: "status update".to_string(),
            enabled: true,
            next_run_at: Some(two_hours_ago.timestamp_millis()),
        });

        let executed = runner.run_missed_on_startup().await.unwrap();
        assert_eq!(executed, 1);

        let runs = store.runs.lock().unwrap();
        assert_eq!(runs.len(), 1);

        // Next run should be scheduled in the future
        let routines = store.routines.lock().unwrap();
        let next = routines[0].next_run_at.unwrap();
        assert!(next > Utc::now().timestamp_millis());
    }
}
