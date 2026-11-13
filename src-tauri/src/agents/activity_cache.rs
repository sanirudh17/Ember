//! Last authoritative hook phase, persisted even while Ember is closed.
//! No prompts/tool arguments are stored. PID plus creation time prevents stale
//! session files from creating holds after a process exits or its PID is reused.
use super::{
    process::process_start_time,
    registry::{HookEvent, HookMessage, Registry, unix_now},
};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Instant;

#[derive(Serialize, Deserialize)]
struct Cached {
    birth: u64,
    message: HookMessage,
}

fn directory() -> std::path::PathBuf {
    crate::paths::config_dir().join("activity-cache")
}

pub fn record(message: &HookMessage) {
    record_at(&directory(), message);
}
fn record_at(dir: &Path, message: &HookMessage) {
    let Some(pid) = message.pid else { return };
    let Some(birth) = process_start_time(pid) else {
        return;
    };
    let mut hash = DefaultHasher::new();
    (message.agent, message.session_id.as_deref().unwrap_or("")).hash(&mut hash);
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join(format!(
        "{}-{pid}-{:016x}.json",
        message.agent.id(),
        hash.finish()
    ));
    let lock_path = path.with_extension("lock");
    let Ok(lock) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
    else {
        return;
    };
    let deadline = Instant::now() + std::time::Duration::from_millis(50);
    loop {
        if lock.try_lock().is_ok() {
            break;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let previous = read(&path).filter(|c| c.birth == birth);
    if message.event == HookEvent::End {
        let _ = std::fs::remove_file(&path);
        return;
    }
    let phase = reduce(previous.as_ref().map(|c| c.message.event), message.event);
    let Some(phase) = phase else { return };
    let mut saved = message.clone();
    saved.event = phase;
    if saved.cwd.as_ref().is_none_or(|v| v.is_empty()) {
        saved.cwd = previous.as_ref().and_then(|p| p.message.cwd.clone());
    }
    if let Ok(bytes) = serde_json::to_vec(&Cached {
        birth,
        message: saved,
    }) {
        let _ = crate::settings::write_atomic(&path, &bytes);
    }
}

fn reduce(previous: Option<HookEvent>, event: HookEvent) -> Option<HookEvent> {
    match event {
        HookEvent::End => None,
        HookEvent::Heartbeat => previous,
        HookEvent::Resume => match previous {
            Some(HookEvent::Waiting) => Some(HookEvent::Working),
            other => other,
        },
        other => Some(other),
    }
}
fn read(path: &Path) -> Option<Cached> {
    if std::fs::metadata(path).ok()?.len() > 64 * 1024 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
pub fn restore(registry: &mut Registry, now: Instant) {
    restore_from(&directory(), registry, now);
}
fn restore_from(dir: &Path, registry: &mut Registry, now: Instant) {
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for path in files
        .flatten()
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .take(1024)
    {
        let Some(mut cached) = read(&path) else {
            continue;
        };
        if cached.message.pid.and_then(process_start_time) != Some(cached.birth) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        // Liveness is checked against process creation, so a live hour-long
        // reasoning turn remains valid regardless of when Ember was reopened.
        cached.message.ts = unix_now();
        registry.on_hook(cached.message, now);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_callbacks_cannot_undo_cancellation_or_invent_a_session() {
        assert_eq!(reduce(None, HookEvent::Heartbeat), None);
        assert_eq!(reduce(None, HookEvent::Resume), None);
        assert_eq!(
            reduce(Some(HookEvent::Idle), HookEvent::Resume),
            Some(HookEvent::Idle)
        );
        assert_eq!(
            reduce(Some(HookEvent::Waiting), HookEvent::Resume),
            Some(HookEvent::Working)
        );
        assert_eq!(
            reduce(Some(HookEvent::Working), HookEvent::Heartbeat),
            Some(HookEvent::Working)
        );
        assert_eq!(
            reduce(Some(HookEvent::Idle), HookEvent::Heartbeat),
            Some(HookEvent::Idle)
        );
    }
    #[test]
    fn process_identity_is_available_only_for_a_live_process() {
        assert!(process_start_time(std::process::id()).is_some());
        assert!(process_start_time(u32::MAX).is_none());
    }
    #[test]
    fn a_short_cache_lock_does_not_drop_cancellation() {
        let dir =
            std::env::temp_dir().join(format!("ember-cache-contention-{}", std::process::id()));
        let mut msg = HookMessage {
            agent: super::super::AgentId::Codex,
            event: HookEvent::Working,
            session_id: Some("contention".into()),
            cwd: None,
            pid: Some(std::process::id()),
            ts: unix_now(),
        };
        record_at(&dir, &msg);
        let path = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|f| f.path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .unwrap();
        lock.try_lock().unwrap();
        msg.event = HookEvent::Idle;
        let destination = dir.clone();
        let writer = std::thread::spawn(move || record_at(&destination, &msg));
        std::thread::sleep(std::time::Duration::from_millis(8));
        drop(lock);
        writer.join().unwrap();
        assert_eq!(read(&path).unwrap().message.event, HookEvent::Idle);
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_agent_uses_the_same_reopen_wait_cancel_and_end_contract() {
        let dir =
            std::env::temp_dir().join(format!("ember-all-agent-cache-{}", std::process::id()));
        let now = Instant::now();
        for agent in super::super::AgentId::ALL {
            let mut message = HookMessage {
                agent,
                event: HookEvent::Working,
                session_id: Some("contract".into()),
                cwd: None,
                pid: Some(std::process::id()),
                ts: unix_now(),
            };
            let mut recovered = |events: &[HookEvent]| {
                for event in events {
                    message.event = *event;
                    record_at(&dir, &message);
                }
                let mut registry = Registry::new();
                restore_from(&dir, &mut registry, now);
                registry
            };
            assert!(
                recovered(&[HookEvent::Working, HookEvent::Heartbeat]).any_working(|_| true),
                "{}",
                agent.id()
            );
            assert!(!recovered(&[HookEvent::Waiting, HookEvent::Heartbeat]).any_working(|_| true));
            assert!(recovered(&[HookEvent::Resume]).any_working(|_| true));
            assert!(
                !recovered(&[HookEvent::Idle, HookEvent::Resume, HookEvent::Heartbeat])
                    .any_working(|_| true)
            );
            assert!(
                recovered(&[HookEvent::End, HookEvent::Heartbeat, HookEvent::Resume]).is_empty()
            );
        }
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reopen_recovers_live_work_and_cancellation_but_not_pid_reuse() {
        let dir = std::env::temp_dir().join(format!(
            "ember-cache-test-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let mut msg = HookMessage {
            agent: super::super::AgentId::OpenCode,
            event: HookEvent::Working,
            session_id: Some("restart".into()),
            cwd: Some("project".into()),
            pid: Some(std::process::id()),
            ts: 1,
        };
        record_at(&dir, &msg);
        let now = Instant::now();
        let mut registry = Registry::new();
        restore_from(&dir, &mut registry, now);
        assert!(
            registry.any_working(|_| true),
            "even an old live reasoning phase survives reopening"
        );
        msg.event = HookEvent::Idle;
        record_at(&dir, &msg);
        msg.event = HookEvent::Resume;
        record_at(&dir, &msg);
        let mut registry = Registry::new();
        restore_from(&dir, &mut registry, now);
        assert!(!registry.any_working(|_| true));
        let path = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|f| f.path())
            .find(|p| p.extension().is_some_and(|e| e == "json"))
            .unwrap();
        let mut cached = read(&path).unwrap();
        cached.birth -= 1;
        cached.message.event = HookEvent::Working;
        std::fs::write(&path, serde_json::to_vec(&cached).unwrap()).unwrap();
        let mut registry = Registry::new();
        restore_from(&dir, &mut registry, now);
        assert!(registry.is_empty());
        assert!(!path.exists());
        // Only this test's newly-created temporary directory is removed.
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
