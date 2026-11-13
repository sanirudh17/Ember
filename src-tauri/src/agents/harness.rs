//! Read-only T3 Code / GOAT session metadata. Their Antigravity ACP runtimes
//! intentionally isolate GEMINI_HOME and never load the normal CLI hooks.
//! No conversation messages, prompts, tokens or credentials are queried.
use super::{
    AgentId, folder_name,
    process::process_start_time,
    registry::{HookEvent, HookMessage, Registry, unix_now},
};
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct HarnessReader {
    known: HashMap<PathBuf, Known>,
}
struct Known {
    sessions: HashMap<(AgentId, String), HookMessage>,
    last_good: Instant,
}
#[derive(Debug)]
struct Row {
    id: String,
    native_id: Option<String>,
    driver: String,
    cwd: String,
    updated: String,
    working: bool,
}

fn roots(mut extras: Vec<PathBuf>) -> Vec<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        for name in [".t3", ".goat", ".t3code"] {
            for suffix in ["userdata", "dev", ""] {
                extras.push(home.join(name).join(suffix));
            }
        }
        // Packaged GOAT stable/nightly deliberately isolate their homes.
        for channel in ["desktop", "desktop-nightly"] {
            extras.push(home.join(".goat").join(channel).join("userdata"));
        }
    }
    for key in ["T3CODE_HOME", "GOAT_HOME"] {
        if let Some(home) = std::env::var_os(key) {
            for suffix in ["userdata", "dev", ""] {
                extras.push(PathBuf::from(&home).join(suffix));
            }
        }
    }
    extras.sort();
    extras.dedup();
    extras
}
fn agent(driver: &str) -> Option<AgentId> {
    match driver {
        "antigravity" | "antigravity-cli" => Some(AgentId::Antigravity),
        "codex" => Some(AgentId::Codex),
        "claude-code" | "claudeCode" | "claude" => Some(AgentId::ClaudeCode),
        "opencode" | "opencode2" => Some(AgentId::OpenCode),
        "pi" => Some(AgentId::Pi),
        "oh-my-pi" | "omp" => Some(AgentId::OhMyPi),
        "cline" => Some(AgentId::Cline),
        _ => None,
    }
}
fn stamp(text: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|d| d.timestamp().max(0) as u64)
}
fn owner(root: &Path, alive: &HashSet<u32>) -> Option<(u32, u64)> {
    let path = root.join("server-runtime.json");
    if std::fs::metadata(&path).ok()?.len() > 64 * 1024 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let pid = u32::try_from(v.get("pid")?.as_u64()?).ok()?;
    if !alive.contains(&pid) {
        return None;
    }
    let birth = process_start_time(pid)?;
    let declared = stamp(v.get("startedAt")?.as_str()?)?;
    // A runtime file from an older server must not bind to a reused PID.
    if declared.saturating_add(2) < birth || declared > birth.saturating_add(3600) {
        return None;
    }
    Some((pid, birth))
}
fn rows(db: &Connection) -> rusqlite::Result<Vec<Row>> {
    let v2:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='orchestration_v2_projection_provider_sessions')",[],|r|r.get(0))?;
    let sql = if v2 {
        r#"
      SELECT t.provider_thread_id, COALESCE(json_extract(s.payload_json,'$.driver'),s.provider),
        COALESCE(json_extract(s.payload_json,'$.cwd'),json_extract(s.payload_json,'$.workspace.path'),'T3 / GOAT'),MAX(s.updated_at,t.updated_at),
        EXISTS(SELECT 1 FROM orchestration_v2_projection_runs r
          WHERE r.provider_thread_id=t.provider_thread_id AND r.completed_at IS NULL
            AND r.status IN ('starting','running','waiting')
            AND NOT EXISTS(SELECT 1 FROM orchestration_v2_projection_runtime_requests q
              WHERE q.thread_id=r.thread_id AND q.resolved_at IS NULL AND q.status IN ('pending','waiting','open'))),
        json_extract(t.payload_json,'$.nativeThreadRef.nativeId')
      FROM orchestration_v2_projection_provider_sessions s
      JOIN orchestration_v2_projection_provider_threads t ON t.provider_session_id=s.provider_session_id
      WHERE s.status IN ('starting','running','waiting','ready') AND t.status IN ('idle','active') LIMIT 256
    "#
    } else {
        r#"
      SELECT s.thread_id,s.provider_name,COALESCE(t.worktree_path,p.workspace_root,'T3 / GOAT'),r.last_seen_at,
        s.active_turn_id IS NOT NULL AND s.status IN ('running','busy')
        AND NOT EXISTS(SELECT 1 FROM projection_pending_approvals a WHERE a.thread_id=s.thread_id AND a.resolved_at IS NULL AND a.status='pending'),NULL
      FROM projection_thread_sessions s
      JOIN provider_session_runtime r ON r.thread_id=s.thread_id
      LEFT JOIN projection_threads t ON t.thread_id=s.thread_id
      LEFT JOIN projection_projects p ON p.project_id=t.project_id
      WHERE r.status='running' AND t.deleted_at IS NULL LIMIT 256
    "#
    };
    let mut query = db.prepare(sql)?;
    query
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                native_id: r.get(5)?,
                driver: r.get(1)?,
                cwd: r.get(2)?,
                updated: r.get(3)?,
                working: r.get(4)?,
            })
        })?
        .collect()
}
impl HarnessReader {
    pub fn poll(
        &mut self,
        registry: &mut Registry,
        alive: &HashSet<u32>,
        extras: Vec<PathBuf>,
        now: Instant,
    ) {
        let mut found = roots(extras);
        found.extend(self.known.keys().cloned());
        found.sort();
        found.dedup();
        for root in found {
            let identity = owner(&root, alive);
            let Some((pid, birth)) = identity else {
                self.clear(&root, registry, now);
                continue;
            };
            let result = Connection::open_with_flags(
                root.join("statev2.sqlite"),
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .and_then(|db| {
                db.busy_timeout(Duration::from_millis(10))?;
                rows(&db)
            });
            match result {
                Ok(rows) => {
                    let mut next = HashMap::new();
                    for row in rows {
                        // Restored database history alone does not prove that this server owns the session.
                        if stamp(&row.updated).is_none_or(|t| t.saturating_add(2) < birth) {
                            continue;
                        }
                        let Some(agent) = agent(&row.driver) else {
                            continue;
                        };
                        let id = row
                            .native_id
                            .filter(|id| !id.is_empty())
                            .unwrap_or_else(|| {
                                format!("harness:{}:{}", root.to_string_lossy(), row.id)
                            });
                        let message = HookMessage {
                            agent,
                            event: if row.working {
                                HookEvent::Working
                            } else {
                                HookEvent::Idle
                            },
                            session_id: Some(id.clone()),
                            cwd: Some(folder_name(&row.cwd)),
                            pid: Some(pid),
                            ts: unix_now(),
                        };
                        let key = (agent, id);
                        if self
                            .known
                            .get(&root)
                            .and_then(|s| s.sessions.get(&key))
                            .is_none_or(|prior| prior.event != message.event)
                        {
                            crate::log::append(
                                "HARNESS",
                                &format!(
                                    "{} {:?} from live local session metadata",
                                    agent.id(),
                                    message.event
                                ),
                            );
                        }
                        registry.on_hook(message.clone(), now);
                        next.insert(key, message);
                    }
                    if let Some(old) = self.known.remove(&root) {
                        for (key, mut msg) in old.sessions {
                            if !next.contains_key(&key)
                                && registry
                                    .get(msg.agent, msg.session_id.as_deref().unwrap_or(""))
                                    .is_some_and(|s| s.pid == msg.pid)
                            {
                                msg.event = HookEvent::End;
                                registry.on_hook(msg, now);
                            }
                        }
                    }
                    self.known.insert(
                        root,
                        Known {
                            sessions: next,
                            last_good: now,
                        },
                    );
                }
                Err(_) => {
                    // One busy WAL read must not release ongoing work. A lost/corrupt
                    // source is bounded and logged rather than trusted indefinitely.
                    if self
                        .known
                        .get(&root)
                        .is_some_and(|s| now.duration_since(s.last_good) > Duration::from_secs(30))
                    {
                        crate::log::append(
                            "HARNESS",
                            "local session metadata unavailable for 30s; retiring its stale records",
                        );
                        self.clear(&root, registry, now);
                    }
                }
            }
        }
    }
    fn clear(&mut self, root: &Path, registry: &mut Registry, now: Instant) {
        if let Some(old) = self.known.remove(root) {
            for mut msg in old.sessions.into_values() {
                msg.event = HookEvent::End;
                if registry
                    .get(msg.agent, msg.session_id.as_deref().unwrap_or(""))
                    .is_some_and(|s| s.pid == msg.pid)
                {
                    registry.on_hook(msg, now);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_harness_requires_a_live_turn_and_suspends_for_user_input() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
          CREATE TABLE projection_thread_sessions(thread_id TEXT,status TEXT,provider_name TEXT,active_turn_id TEXT);
          CREATE TABLE provider_session_runtime(thread_id TEXT,status TEXT,last_seen_at TEXT);
          CREATE TABLE projection_threads(thread_id TEXT,project_id TEXT,worktree_path TEXT,deleted_at TEXT);
          CREATE TABLE projection_projects(project_id TEXT,workspace_root TEXT);
          CREATE TABLE projection_pending_approvals(thread_id TEXT,status TEXT,resolved_at TEXT);
          INSERT INTO projection_thread_sessions VALUES('one','running','antigravity','turn');
          INSERT INTO provider_session_runtime VALUES('one','running','2026-11-12T10:00:00Z');
          INSERT INTO projection_threads VALUES('one','project',NULL,NULL);
          INSERT INTO projection_projects VALUES('project','C:/project');
        "#).unwrap();
        assert!(rows(&db).unwrap()[0].working);
        db.execute(
            "INSERT INTO projection_pending_approvals VALUES('one','pending',NULL)",
            [],
        )
        .unwrap();
        assert!(!rows(&db).unwrap()[0].working);
        db.execute(
            "UPDATE projection_pending_approvals SET resolved_at='now'",
            [],
        )
        .unwrap();
        assert!(rows(&db).unwrap()[0].working);
        db.execute(
            "UPDATE projection_thread_sessions SET active_turn_id=NULL",
            [],
        )
        .unwrap();
        assert!(
            !rows(&db).unwrap()[0].working,
            "open transport without an active turn is idle"
        );
        db.execute("UPDATE provider_session_runtime SET status='stopped'", [])
            .unwrap();
        assert!(rows(&db).unwrap().is_empty());
    }
    #[test]
    fn runtime_file_cannot_bind_to_a_dead_or_reused_pid() {
        let root = std::env::temp_dir().join(format!("ember-harness-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let pid = std::process::id();
        let birth = process_start_time(pid).unwrap();
        let alive = HashSet::from([pid]);
        let write = |time| {
            let v = serde_json::json!({"pid":pid,"startedAt":chrono::DateTime::from_timestamp(time as i64,0).unwrap().to_rfc3339()});
            std::fs::write(root.join("server-runtime.json"), v.to_string()).unwrap();
        };
        write(birth);
        assert_eq!(owner(&root, &alive), Some((pid, birth)));
        assert!(owner(&root, &HashSet::new()).is_none());
        write(birth - 10);
        assert!(owner(&root, &alive).is_none());
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn v2_reasoning_background_wait_and_question_states_follow_the_harness() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(r#"
         CREATE TABLE orchestration_v2_projection_provider_sessions(provider_session_id TEXT,provider TEXT,status TEXT,payload_json TEXT,updated_at TEXT);
         CREATE TABLE orchestration_v2_projection_provider_threads(provider_session_id TEXT,provider_thread_id TEXT,status TEXT,payload_json TEXT,updated_at TEXT);
         CREATE TABLE orchestration_v2_projection_runs(thread_id TEXT,provider_thread_id TEXT,status TEXT,completed_at TEXT);
         CREATE TABLE orchestration_v2_projection_runtime_requests(thread_id TEXT,status TEXT,resolved_at TEXT);
         INSERT INTO orchestration_v2_projection_provider_sessions VALUES('acp','provider-instance','running','{"driver":"antigravity"}','2026-11-12T10:00:00Z');
         INSERT INTO orchestration_v2_projection_provider_threads VALUES('acp','pt','active','{"nativeThreadRef":{"nativeId":"native-one"}}','2026-11-12T10:00:00Z');
         INSERT INTO orchestration_v2_projection_runs VALUES('thread','pt','running',NULL);
        "#).unwrap();
        assert!(
            rows(&db).unwrap()[0].working,
            "reasoning and shell work are part of a running turn"
        );
        db.execute(
            "UPDATE orchestration_v2_projection_runs SET status='waiting'",
            [],
        )
        .unwrap();
        assert!(
            rows(&db).unwrap()[0].working,
            "waiting on background work is still work"
        );
        db.execute("INSERT INTO orchestration_v2_projection_runtime_requests VALUES('thread','pending',NULL)",[]).unwrap();
        assert!(
            !rows(&db).unwrap()[0].working,
            "waiting on user input is idle"
        );
        db.execute(
            "UPDATE orchestration_v2_projection_runtime_requests SET resolved_at='now'",
            [],
        )
        .unwrap();
        assert!(rows(&db).unwrap()[0].working);
        db.execute(
            "UPDATE orchestration_v2_projection_runs SET status='interrupted',completed_at='now'",
            [],
        )
        .unwrap();
        assert!(!rows(&db).unwrap()[0].working);
        assert_eq!(
            agent(&rows(&db).unwrap()[0].driver),
            Some(AgentId::Antigravity)
        );
        db.execute_batch(r#"
          INSERT INTO orchestration_v2_projection_provider_threads VALUES('acp','pt2','active','{"nativeThreadRef":{"nativeId":"native-two"}}','2026-11-12T10:00:00Z');
          INSERT INTO orchestration_v2_projection_runs VALUES('thread2','pt2','running',NULL);
        "#).unwrap();
        let sessions = rows(&db).unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(
            !sessions
                .iter()
                .find(|r| r.native_id.as_deref() == Some("native-one"))
                .unwrap()
                .working
        );
        assert!(
            sessions
                .iter()
                .find(|r| r.native_id.as_deref() == Some("native-two"))
                .unwrap()
                .working,
            "one interrupted thread must not idle a sibling sharing its server"
        );
        db.execute("UPDATE orchestration_v2_projection_provider_threads SET status='closed' WHERE provider_thread_id='pt2'",[]).unwrap();
        assert_eq!(rows(&db).unwrap().len(), 1);
    }
}
