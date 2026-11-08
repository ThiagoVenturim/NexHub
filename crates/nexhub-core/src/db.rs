//! SQLite schema, migrations, and typed queries.

use crate::model::{
    Agent, AgentState, Connection, Floor, FloorStatus, Group, Message, MessageKind, MessageState,
    ModelError, Note, Routine, Run, Workspace,
};
use rusqlite::{params, Connection as SqliteConnection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("model error: {0}")]
    Model(#[from] ModelError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

const MIGRATION_1: &str = r#"
CREATE TABLE groups (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, repo_path TEXT NOT NULL,
  group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
  layout_json TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL
);
CREATE TABLE floors (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL, branch TEXT NOT NULL, path TEXT NOT NULL,
  base_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active','merged','removed')),
  created_at INTEGER NOT NULL, UNIQUE (workspace_id, name)
);
CREATE TABLE agents (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  floor_id TEXT REFERENCES floors(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  preset TEXT NOT NULL, role TEXT,
  cwd TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('stopped','starting','running','busy','waiting_quota','exited','error')),
  quota_reset_at INTEGER,
  last_prompt TEXT,
  created_at INTEGER NOT NULL, UNIQUE (workspace_id, name)
);
CREATE TABLE messages (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_agent TEXT,
  to_agent TEXT,
  kind TEXT NOT NULL CHECK (kind IN ('ask','notify','broadcast','reply')),
  reply_to TEXT REFERENCES messages(id),
  body TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('queued','delivered','answered','expired','failed')),
  created_at INTEGER NOT NULL, delivered_at INTEGER, answered_at INTEGER
);
CREATE INDEX messages_inbox ON messages (to_agent, state);
CREATE TABLE notes (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL, body TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
  UNIQUE (workspace_id, title)
);
CREATE TABLE connections (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_id TEXT NOT NULL, to_id TEXT NOT NULL,
  PRIMARY KEY (workspace_id, from_id, to_id)
);
CREATE TABLE routines (
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL, schedule TEXT NOT NULL,
  target_agent TEXT,
  command TEXT,
  body TEXT NOT NULL DEFAULT '', enabled INTEGER NOT NULL DEFAULT 1,
  next_run_at INTEGER, UNIQUE (workspace_id, name),
  CHECK ((target_agent IS NULL) <> (command IS NULL))
);
CREATE TABLE runs (
  id TEXT PRIMARY KEY, routine_id TEXT NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
  started_at INTEGER NOT NULL, finished_at INTEGER, exit_code INTEGER, output_tail TEXT
);
CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL,
  workspace_id TEXT, kind TEXT NOT NULL, data_json TEXT NOT NULL
);
"#;

/// Thread-safe SQLite database manager for NexHub.
#[derive(Debug)]
pub struct Db {
    conn: Mutex<SqliteConnection>,
}

impl Db {
    /// Opens a SQLite database at the specified path, ensures WAL mode and foreign keys,
    /// and runs any pending migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = SqliteConnection::open(path)?;
        Self::init(conn)
    }

    /// Opens an in-memory SQLite database, configured with foreign keys and migration 1 applied.
    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = SqliteConnection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: SqliteConnection) -> Result<Self, DbError> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

        let user_version: u32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

        if user_version == 0 {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(MIGRATION_1)?;
            tx.execute_batch("PRAGMA user_version = 1;")?;
            tx.commit()?;
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the current schema `user_version`.
    pub fn user_version(&self) -> Result<u32, DbError> {
        let conn = self.conn.lock().unwrap();
        let v: u32 = conn.query_row("PRAGMA user_version;", [], |r| r.get(0))?;
        Ok(v)
    }

    // --- Workspaces ---

    pub fn insert_workspace(&self, ws: &Workspace) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO workspaces (id, name, repo_path, group_id, layout_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                ws.id,
                ws.name,
                ws.repo_path,
                ws.group_id,
                ws.layout_json,
                ws.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_workspace(&self, id: &str) -> Result<Option<Workspace>, DbError> {
        let conn = self.conn.lock().unwrap();
        let ws = conn
            .query_row(
                "SELECT id, name, repo_path, group_id, layout_json, created_at
                 FROM workspaces WHERE id = ?1;",
                params![id],
                |row| {
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        repo_path: row.get(2)?,
                        group_id: row.get(3)?,
                        layout_json: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(ws)
    }

    pub fn get_workspace_by_name(&self, name: &str) -> Result<Option<Workspace>, DbError> {
        let conn = self.conn.lock().unwrap();
        let ws = conn
            .query_row(
                "SELECT id, name, repo_path, group_id, layout_json, created_at
                 FROM workspaces WHERE name = ?1;",
                params![name],
                |row| {
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        repo_path: row.get(2)?,
                        group_id: row.get(3)?,
                        layout_json: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(ws)
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, repo_path, group_id, layout_json, created_at
             FROM workspaces ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                repo_path: row.get(2)?,
                group_id: row.get(3)?,
                layout_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn update_workspace_layout(&self, id: &str, layout_json: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE workspaces SET layout_json = ?2 WHERE id = ?1;",
            params![id, layout_json],
        )?;
        Ok(())
    }

    pub fn delete_workspace(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM workspaces WHERE id = ?1;", params![id])?;
        Ok(count > 0)
    }

    // --- Groups ---

    pub fn insert_group(&self, group: &Group) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO groups (id, name, position) VALUES (?1, ?2, ?3);",
            params![group.id, group.name, group.position],
        )?;
        Ok(())
    }

    pub fn list_groups(&self) -> Result<Vec<Group>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, position FROM groups ORDER BY position ASC, name ASC;")?;
        let rows = stmt.query_map([], |row| {
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                position: row.get(2)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // --- Floors ---

    pub fn insert_floor(&self, floor: &Floor) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO floors (id, workspace_id, name, branch, path, base_ref, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
            params![
                floor.id,
                floor.workspace_id,
                floor.name,
                floor.branch,
                floor.path,
                floor.base_ref,
                floor.status.as_str(),
                floor.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_floor(&self, id: &str) -> Result<Option<Floor>, DbError> {
        let conn = self.conn.lock().unwrap();
        let floor = conn
            .query_row(
                "SELECT id, workspace_id, name, branch, path, base_ref, status, created_at
                 FROM floors WHERE id = ?1;",
                params![id],
                |row| {
                    let status_str: String = row.get(6)?;
                    let status = status_str.parse::<FloorStatus>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(Floor {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        name: row.get(2)?,
                        branch: row.get(3)?,
                        path: row.get(4)?,
                        base_ref: row.get(5)?,
                        status,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(floor)
    }

    pub fn get_floor_by_name(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<Option<Floor>, DbError> {
        let conn = self.conn.lock().unwrap();
        let floor = conn
            .query_row(
                "SELECT id, workspace_id, name, branch, path, base_ref, status, created_at
                 FROM floors WHERE workspace_id = ?1 AND name = ?2;",
                params![workspace_id, name],
                |row| {
                    let status_str: String = row.get(6)?;
                    let status = status_str.parse::<FloorStatus>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(Floor {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        name: row.get(2)?,
                        branch: row.get(3)?,
                        path: row.get(4)?,
                        base_ref: row.get(5)?,
                        status,
                        created_at: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(floor)
    }

    pub fn list_floors(&self, workspace_id: &str) -> Result<Vec<Floor>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, name, branch, path, base_ref, status, created_at
             FROM floors WHERE workspace_id = ?1 ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            let status_str: String = row.get(6)?;
            let status = status_str.parse::<FloorStatus>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Floor {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                name: row.get(2)?,
                branch: row.get(3)?,
                path: row.get(4)?,
                base_ref: row.get(5)?,
                status,
                created_at: row.get(7)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn set_floor_status(&self, id: &str, status: FloorStatus) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE floors SET status = ?2 WHERE id = ?1;",
            params![id, status.as_str()],
        )?;
        Ok(())
    }

    pub fn delete_floor(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM floors WHERE id = ?1;", params![id])?;
        Ok(count > 0)
    }

    // --- Agents ---

    pub fn insert_agent(&self, agent: &Agent) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agents (id, workspace_id, floor_id, name, preset, role, cwd, state, quota_reset_at, last_prompt, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);",
            params![
                agent.id,
                agent.workspace_id,
                agent.floor_id,
                agent.name,
                agent.preset,
                agent.role,
                agent.cwd,
                agent.state.as_str(),
                agent.quota_reset_at,
                agent.last_prompt,
                agent.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_agent(&self, id: &str) -> Result<Option<Agent>, DbError> {
        let conn = self.conn.lock().unwrap();
        let agent = conn
            .query_row(
                "SELECT id, workspace_id, floor_id, name, preset, role, cwd, state, quota_reset_at, last_prompt, created_at
                 FROM agents WHERE id = ?1;",
                params![id],
                |row| {
                    let state_str: String = row.get(7)?;
                    let state = state_str
                        .parse::<AgentState>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?;
                    Ok(Agent {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        floor_id: row.get(2)?,
                        name: row.get(3)?,
                        preset: row.get(4)?,
                        role: row.get(5)?,
                        cwd: row.get(6)?,
                        state,
                        quota_reset_at: row.get(8)?,
                        last_prompt: row.get(9)?,
                        created_at: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(agent)
    }

    pub fn get_agent_by_name(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<Option<Agent>, DbError> {
        let conn = self.conn.lock().unwrap();
        let agent = conn
            .query_row(
                "SELECT id, workspace_id, floor_id, name, preset, role, cwd, state, quota_reset_at, last_prompt, created_at
                 FROM agents WHERE workspace_id = ?1 AND name = ?2;",
                params![workspace_id, name],
                |row| {
                    let state_str: String = row.get(7)?;
                    let state = state_str
                        .parse::<AgentState>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?;
                    Ok(Agent {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        floor_id: row.get(2)?,
                        name: row.get(3)?,
                        preset: row.get(4)?,
                        role: row.get(5)?,
                        cwd: row.get(6)?,
                        state,
                        quota_reset_at: row.get(8)?,
                        last_prompt: row.get(9)?,
                        created_at: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(agent)
    }

    pub fn list_agents(&self, workspace_id: &str) -> Result<Vec<Agent>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, floor_id, name, preset, role, cwd, state, quota_reset_at, last_prompt, created_at
             FROM agents WHERE workspace_id = ?1 ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            let state_str: String = row.get(7)?;
            let state = state_str.parse::<AgentState>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Agent {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                floor_id: row.get(2)?,
                name: row.get(3)?,
                preset: row.get(4)?,
                role: row.get(5)?,
                cwd: row.get(6)?,
                state,
                quota_reset_at: row.get(8)?,
                last_prompt: row.get(9)?,
                created_at: row.get(10)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn set_agent_state(&self, id: &str, state: AgentState) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET state = ?2 WHERE id = ?1;",
            params![id, state.as_str()],
        )?;
        Ok(())
    }

    pub fn set_agent_quota_reset(
        &self,
        id: &str,
        quota_reset_at: Option<i64>,
        last_prompt: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE agents SET quota_reset_at = ?2, last_prompt = ?3 WHERE id = ?1;",
            params![id, quota_reset_at, last_prompt],
        )?;
        Ok(())
    }

    pub fn delete_agent(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM agents WHERE id = ?1;", params![id])?;
        Ok(count > 0)
    }

    // --- Messages ---

    pub fn insert_message(&self, msg: &Message) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, workspace_id, from_agent, to_agent, kind, reply_to, body, state, created_at, delivered_at, answered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11);",
            params![
                msg.id,
                msg.workspace_id,
                msg.from_agent,
                msg.to_agent,
                msg.kind.as_str(),
                msg.reply_to,
                msg.body,
                msg.state.as_str(),
                msg.created_at,
                msg.delivered_at,
                msg.answered_at
            ],
        )?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>, DbError> {
        let conn = self.conn.lock().unwrap();
        let msg = conn
            .query_row(
                "SELECT id, workspace_id, from_agent, to_agent, kind, reply_to, body, state, created_at, delivered_at, answered_at
                 FROM messages WHERE id = ?1;",
                params![id],
                |row| {
                    let kind_str: String = row.get(4)?;
                    let kind = kind_str
                        .parse::<MessageKind>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
                    let state_str: String = row.get(7)?;
                    let state = state_str
                        .parse::<MessageState>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?;
                    Ok(Message {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        from_agent: row.get(2)?,
                        to_agent: row.get(3)?,
                        kind,
                        reply_to: row.get(5)?,
                        body: row.get(6)?,
                        state,
                        created_at: row.get(8)?,
                        delivered_at: row.get(9)?,
                        answered_at: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(msg)
    }

    pub fn mark_message(
        &self,
        id: &str,
        state: MessageState,
        delivered_at: Option<i64>,
        answered_at: Option<i64>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE messages
             SET state = ?2,
                 delivered_at = COALESCE(?3, delivered_at),
                 answered_at = COALESCE(?4, answered_at)
             WHERE id = ?1;",
            params![id, state.as_str(), delivered_at, answered_at],
        )?;
        Ok(())
    }

    pub fn next_queued_for(
        &self,
        agent_name: &str,
        workspace_id: &str,
    ) -> Result<Option<Message>, DbError> {
        let conn = self.conn.lock().unwrap();
        let msg = conn
            .query_row(
                "SELECT id, workspace_id, from_agent, to_agent, kind, reply_to, body, state, created_at, delivered_at, answered_at
                 FROM messages
                 WHERE workspace_id = ?1 AND to_agent = ?2 AND state = 'queued'
                 ORDER BY created_at ASC LIMIT 1;",
                params![workspace_id, agent_name],
                |row| {
                    let kind_str: String = row.get(4)?;
                    let kind = kind_str
                        .parse::<MessageKind>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
                    let state_str: String = row.get(7)?;
                    let state = state_str
                        .parse::<MessageState>()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?;
                    Ok(Message {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        from_agent: row.get(2)?,
                        to_agent: row.get(3)?,
                        kind,
                        reply_to: row.get(5)?,
                        body: row.get(6)?,
                        state,
                        created_at: row.get(8)?,
                        delivered_at: row.get(9)?,
                        answered_at: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(msg)
    }

    pub fn list_inbox(
        &self,
        agent_name: &str,
        workspace_id: &str,
    ) -> Result<Vec<Message>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, from_agent, to_agent, kind, reply_to, body, state, created_at, delivered_at, answered_at
             FROM messages
             WHERE workspace_id = ?1 AND (to_agent = ?2 OR to_agent IS NULL)
             ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map(params![workspace_id, agent_name], |row| {
            let kind_str: String = row.get(4)?;
            let kind = kind_str.parse::<MessageKind>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let state_str: String = row.get(7)?;
            let state = state_str.parse::<MessageState>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Message {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                from_agent: row.get(2)?,
                to_agent: row.get(3)?,
                kind,
                reply_to: row.get(5)?,
                body: row.get(6)?,
                state,
                created_at: row.get(8)?,
                delivered_at: row.get(9)?,
                answered_at: row.get(10)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // --- Notes ---

    pub fn upsert_note(&self, note: &Note) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO notes (id, workspace_id, title, body, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_id, title) DO UPDATE SET
               body = excluded.body,
               updated_at = excluded.updated_at;",
            params![
                note.id,
                note.workspace_id,
                note.title,
                note.body,
                note.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_note(&self, workspace_id: &str, title: &str) -> Result<Option<Note>, DbError> {
        let conn = self.conn.lock().unwrap();
        let note = conn
            .query_row(
                "SELECT id, workspace_id, title, body, updated_at
                 FROM notes WHERE workspace_id = ?1 AND title = ?2;",
                params![workspace_id, title],
                |row| {
                    Ok(Note {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        title: row.get(2)?,
                        body: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(note)
    }

    pub fn list_notes(&self, workspace_id: &str) -> Result<Vec<Note>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, title, body, updated_at
             FROM notes WHERE workspace_id = ?1 ORDER BY title ASC;",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            Ok(Note {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn delete_note(&self, workspace_id: &str, title: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM notes WHERE workspace_id = ?1 AND title = ?2;",
            params![workspace_id, title],
        )?;
        Ok(count > 0)
    }

    // --- Connections ---

    pub fn insert_connection(&self, conn_data: &Connection) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO connections (workspace_id, from_id, to_id)
             VALUES (?1, ?2, ?3);",
            params![conn_data.workspace_id, conn_data.from_id, conn_data.to_id],
        )?;
        Ok(())
    }

    pub fn list_connections(&self, workspace_id: &str) -> Result<Vec<Connection>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT workspace_id, from_id, to_id
             FROM connections WHERE workspace_id = ?1;",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            Ok(Connection {
                workspace_id: row.get(0)?,
                from_id: row.get(1)?,
                to_id: row.get(2)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn delete_connection(
        &self,
        workspace_id: &str,
        from_id: &str,
        to_id: &str,
    ) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM connections WHERE workspace_id = ?1 AND from_id = ?2 AND to_id = ?3;",
            params![workspace_id, from_id, to_id],
        )?;
        Ok(count > 0)
    }

    // --- Routines & Runs ---

    pub fn insert_routine(&self, r: &Routine) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO routines (id, workspace_id, name, schedule, target_agent, command, body, enabled, next_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                r.id,
                r.workspace_id,
                r.name,
                r.schedule,
                r.target_agent,
                r.command,
                r.body,
                if r.enabled { 1 } else { 0 },
                r.next_run_at
            ],
        )?;
        Ok(())
    }

    pub fn get_routine_by_name(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<Option<Routine>, DbError> {
        let conn = self.conn.lock().unwrap();
        let routine = conn
            .query_row(
                "SELECT id, workspace_id, name, schedule, target_agent, command, body, enabled, next_run_at
                 FROM routines WHERE workspace_id = ?1 AND name = ?2;",
                params![workspace_id, name],
                |row| {
                    let enabled_int: i64 = row.get(7)?;
                    Ok(Routine {
                        id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        name: row.get(2)?,
                        schedule: row.get(3)?,
                        target_agent: row.get(4)?,
                        command: row.get(5)?,
                        body: row.get(6)?,
                        enabled: enabled_int != 0,
                        next_run_at: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(routine)
    }

    pub fn list_routines(&self, workspace_id: &str) -> Result<Vec<Routine>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, name, schedule, target_agent, command, body, enabled, next_run_at
             FROM routines WHERE workspace_id = ?1 ORDER BY name ASC;",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            let enabled_int: i64 = row.get(7)?;
            Ok(Routine {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                name: row.get(2)?,
                schedule: row.get(3)?,
                target_agent: row.get(4)?,
                command: row.get(5)?,
                body: row.get(6)?,
                enabled: enabled_int != 0,
                next_run_at: row.get(8)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn set_routine_next_run(&self, id: &str, next_run_at: Option<i64>) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE routines SET next_run_at = ?2 WHERE id = ?1;",
            params![id, next_run_at],
        )?;
        Ok(())
    }

    pub fn delete_routine(&self, id: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM routines WHERE id = ?1;", params![id])?;
        Ok(count > 0)
    }

    pub fn insert_run(&self, run: &Run) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO runs (id, routine_id, started_at, finished_at, exit_code, output_tail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                run.id,
                run.routine_id,
                run.started_at,
                run.finished_at,
                run.exit_code,
                run.output_tail
            ],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        id: &str,
        finished_at: i64,
        exit_code: Option<i32>,
        output_tail: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE runs SET finished_at = ?2, exit_code = ?3, output_tail = ?4 WHERE id = ?1;",
            params![id, finished_at, exit_code, output_tail],
        )?;
        Ok(())
    }

    // --- Events ---

    pub fn insert_event(
        &self,
        at: i64,
        workspace_id: Option<&str>,
        kind: &str,
        data_json: &str,
    ) -> Result<i64, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (at, workspace_id, kind, data_json) VALUES (?1, ?2, ?3, ?4);",
            params![at, workspace_id, kind, data_json],
        )?;
        let id = conn.last_insert_rowid();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_and_reopen_preserves_version() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("nexhub.db");

        // First open: runs migration
        {
            let db = Db::open(&db_path).unwrap();
            assert_eq!(db.user_version().unwrap(), 1);

            let ws = Workspace {
                id: "01J00000000000000000000001".to_string(),
                name: "test-workspace".to_string(),
                repo_path: "/path/to/repo".to_string(),
                group_id: None,
                layout_json: "{}".to_string(),
                created_at: 1000,
            };
            db.insert_workspace(&ws).unwrap();
        }

        // Second open: should not fail and should retain data and user_version
        {
            let db = Db::open(&db_path).unwrap();
            assert_eq!(db.user_version().unwrap(), 1);
            let ws = db
                .get_workspace("01J00000000000000000000001")
                .unwrap()
                .unwrap();
            assert_eq!(ws.name, "test-workspace");
        }
    }

    #[test]
    fn test_check_constraints_reject_invalid_states() {
        let db = Db::open_in_memory().unwrap();

        let ws = Workspace {
            id: "ws-1".to_string(),
            name: "ws-1".to_string(),
            repo_path: "/repo".to_string(),
            group_id: None,
            layout_json: "{}".to_string(),
            created_at: 1000,
        };
        db.insert_workspace(&ws).unwrap();

        // 1. Invalid floor status
        let conn = db.conn.lock().unwrap();
        let res = conn.execute(
            "INSERT INTO floors (id, workspace_id, name, branch, path, base_ref, status, created_at)
             VALUES ('f-1', 'ws-1', 'floor-1', 'layer/floor-1', '/floor', 'origin/develop', 'bogus_status', 1000);",
            [],
        );
        assert!(res.is_err());

        // 2. Invalid agent state
        let res = conn.execute(
            "INSERT INTO agents (id, workspace_id, name, preset, cwd, state, created_at)
             VALUES ('a-1', 'ws-1', 'agent-1', 'agy', '/repo', 'invalid_state', 1000);",
            [],
        );
        assert!(res.is_err());

        // 3. Invalid message kind
        let res = conn.execute(
            "INSERT INTO messages (id, workspace_id, kind, body, state, created_at)
             VALUES ('m-1', 'ws-1', 'invalid_kind', 'body', 'queued', 1000);",
            [],
        );
        assert!(res.is_err());

        // 4. Invalid message state
        let res = conn.execute(
            "INSERT INTO messages (id, workspace_id, kind, body, state, created_at)
             VALUES ('m-1', 'ws-1', 'ask', 'body', 'invalid_state', 1000);",
            [],
        );
        assert!(res.is_err());

        // 5. Invalid routine check constraint: both target_agent and command are null or both not null
        let res = conn.execute(
            "INSERT INTO routines (id, workspace_id, name, schedule, target_agent, command, body, enabled)
             VALUES ('r-1', 'ws-1', 'routine-1', 'every 5m', NULL, NULL, 'body', 1);",
            [],
        );
        assert!(res.is_err());

        let res = conn.execute(
            "INSERT INTO routines (id, workspace_id, name, schedule, target_agent, command, body, enabled)
             VALUES ('r-2', 'ws-1', 'routine-2', 'every 5m', 'target_agent', 'git status', 'body', 1);",
            [],
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_crud_typed_operations() {
        let db = Db::open_in_memory().unwrap();

        // Workspace
        let ws = Workspace {
            id: "ws-1".to_string(),
            name: "repo1".to_string(),
            repo_path: "/repo".to_string(),
            group_id: None,
            layout_json: "{\"zoom\":1}".to_string(),
            created_at: 100,
        };
        db.insert_workspace(&ws).unwrap();
        assert_eq!(db.list_workspaces().unwrap().len(), 1);

        // Floor
        let floor = Floor {
            id: "fl-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "core".to_string(),
            branch: "layer/core".to_string(),
            path: "/repo/floor-core".to_string(),
            base_ref: "origin/develop".to_string(),
            status: FloorStatus::Active,
            created_at: 150,
        };
        db.insert_floor(&floor).unwrap();
        let fetched_floor = db.get_floor_by_name("ws-1", "core").unwrap().unwrap();
        assert_eq!(fetched_floor.status, FloorStatus::Active);

        db.set_floor_status("fl-1", FloorStatus::Merged).unwrap();
        assert_eq!(
            db.get_floor("fl-1").unwrap().unwrap().status,
            FloorStatus::Merged
        );

        // Agent
        let agent = Agent {
            id: "ag-1".to_string(),
            workspace_id: "ws-1".to_string(),
            floor_id: Some("fl-1".to_string()),
            name: "builder".to_string(),
            preset: "agy-auto".to_string(),
            role: Some("floor-builder".to_string()),
            cwd: "/repo".to_string(),
            state: AgentState::Starting,
            quota_reset_at: None,
            last_prompt: None,
            created_at: 200,
        };
        db.insert_agent(&agent).unwrap();
        assert_eq!(db.list_agents("ws-1").unwrap().len(), 1);

        db.set_agent_state("ag-1", AgentState::Running).unwrap();
        assert_eq!(
            db.get_agent("ag-1").unwrap().unwrap().state,
            AgentState::Running
        );

        // Messages & Inbox
        let msg = Message {
            id: "m-1".to_string(),
            workspace_id: "ws-1".to_string(),
            from_agent: None,
            to_agent: Some("builder".to_string()),
            kind: MessageKind::Ask,
            reply_to: None,
            body: "build floor core".to_string(),
            state: MessageState::Queued,
            created_at: 300,
            delivered_at: None,
            answered_at: None,
        };
        db.insert_message(&msg).unwrap();

        let next = db.next_queued_for("builder", "ws-1").unwrap().unwrap();
        assert_eq!(next.id, "m-1");

        db.mark_message("m-1", MessageState::Delivered, Some(350), None)
            .unwrap();
        let delivered = db.get_message("m-1").unwrap().unwrap();
        assert_eq!(delivered.state, MessageState::Delivered);
        assert_eq!(delivered.delivered_at, Some(350));

        // Notes
        let note = Note {
            id: "n-1".to_string(),
            workspace_id: "ws-1".to_string(),
            title: "Scratchpad".to_string(),
            body: "Hello notes".to_string(),
            updated_at: 400,
        };
        db.upsert_note(&note).unwrap();
        let fetched_note = db.get_note("ws-1", "Scratchpad").unwrap().unwrap();
        assert_eq!(fetched_note.body, "Hello notes");

        // Connections
        let conn = Connection {
            workspace_id: "ws-1".to_string(),
            from_id: "ag-1".to_string(),
            to_id: "n-1".to_string(),
        };
        db.insert_connection(&conn).unwrap();
        assert_eq!(db.list_connections("ws-1").unwrap().len(), 1);

        // Routines & Runs
        let routine = Routine {
            id: "r-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "healthcheck".to_string(),
            schedule: "every 5m".to_string(),
            target_agent: None,
            command: Some("git status".to_string()),
            body: "status check".to_string(),
            enabled: true,
            next_run_at: Some(500),
        };
        db.insert_routine(&routine).unwrap();
        assert_eq!(db.list_routines("ws-1").unwrap().len(), 1);

        let run = Run {
            id: "run-1".to_string(),
            routine_id: "r-1".to_string(),
            started_at: 500,
            finished_at: None,
            exit_code: None,
            output_tail: None,
        };
        db.insert_run(&run).unwrap();
        db.finish_run("run-1", 505, Some(0), Some("clean")).unwrap();

        // Events
        let event_id = db
            .insert_event(600, Some("ws-1"), "test_event", "{}")
            .unwrap();
        assert!(event_id > 0);
    }
}
