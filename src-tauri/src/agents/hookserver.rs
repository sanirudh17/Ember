//! The named-pipe hook server.
//!
//! Every installed agent hook runs `"<Ember.exe>" hook <agent> <event>`, which
//! connects to `\\.\pipe\ember-hooks` and writes exactly one JSON line. This
//! module owns the server half: create an instance, wait for a client, read one
//! line, verify the per-install token, and hand a [`HookMessage`] to the engine.
//!
//! `PIPE_REJECT_REMOTE_CLIENTS` plus the token means a remote machine can
//! neither reach nor forge a hook.

use std::sync::mpsc::Sender;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::core::PCWSTR;

use crate::agents::registry::{HookEvent, HookMessage};
use crate::agents::{AgentId, parse_hook_payload};
use crate::paths;

/// One hook message may not exceed this, including the JSON wrapper.
pub const MAX_MESSAGE: usize = 1024 * 1024;

/// The only protocol version Ember speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// The exact one-line JSON frame the hook CLI posts.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HookWire {
    pub v: u32,
    #[serde(default)]
    pub token: String,
    pub agent: String,
    pub event: String,
    /// The agent's own stdin payload, verbatim, as a string.
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub ts: u64,
}

/// `%APPDATA%\Ember\hook-token`, created on first use.
pub fn ensure_token() -> Result<String, String> {
    let path = paths::token_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        let token = text.trim().to_string();
        if token.len() >= 16 {
            return Ok(token);
        }
    }
    let token = random_token();
    crate::settings::write_atomic(&path, token.as_bytes())?;
    Ok(token)
}

/// 32 hex characters from the OS CSPRNG.
pub fn random_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut out = String::with_capacity(32);
    for _ in 0..32 {
        let nibble: u8 = rng.random_range(0..16);
        out.push(std::char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
    }
    out
}

/// Start the pipe server on its own thread. Errors go to the ember log; the
/// app keeps working (process detection still covers every agent).
pub fn spawn(tx: Sender<HookMessage>) {
    let token = match ensure_token() {
        Ok(token) => token,
        Err(e) => {
            eprintln!("ember: could not prepare the hook token: {e}");
            return;
        }
    };
    let _ = std::thread::Builder::new()
        .name("ember-pipe".into())
        .spawn(move || {
            loop {
                if let Err(e) = serve_once(&tx, &token) {
                    crate::log::append("HOOK", &format!("hook pipe: {e}"));
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        });
}

/// How many pipe instances may exist at once. The accept loop hands each
/// connected instance to a short-lived reader and immediately creates the
/// next one; with a single instance, hook bursts dropped events whenever a
/// client reached the pipe during the disconnect/re-create window.
const MAX_INSTANCES: u32 = 8;

/// `HANDLE` is a raw pointer and not `Send`; a connected pipe handle is valid
/// in a reader thread, so the wrapper is safe.
struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}

fn serve_once(tx: &Sender<HookMessage>, token: &str) -> Result<(), String> {
    let name: Vec<u16> = paths::PIPE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            MAX_INSTANCES,
            0,
            64 * 1024,
            0,
            None,
        )
    };
    if handle.is_invalid() || handle == INVALID_HANDLE_VALUE {
        return Err("CreateNamedPipeW failed".into());
    }

    // Blocks until a client connects. ERROR_PIPE_CONNECTED means one was
    // already waiting, which is still success — so the result is informational.
    let _ = unsafe { ConnectNamedPipe(handle, None) };

    let raw = handle; // HANDLE is Copy: the error arm can still close it.
    let tx = tx.clone();
    let token = token.to_string();
    let shared = SendHandle(raw);
    let worker = std::thread::Builder::new()
        .name("ember-pipe-read".into())
        .spawn(move || {
            read_instance(shared, &tx, &token);
        });
    match worker {
        Ok(_) => Ok(()),
        Err(_) => {
            // The closure was dropped with the handle still open; close it on
            // this thread (the spawn error path cannot use the moved value).
            unsafe {
                let _ = DisconnectNamedPipe(handle);
                let _ = CloseHandle(handle);
            }
            Err("could not start a pipe reader".into())
        }
    }
}

/// Read one client's frame, then close the instance. Runs on its own thread
/// so the accept loop can go straight back to `CreateNamedPipeW`.
fn read_instance(handle: SendHandle, tx: &Sender<HookMessage>, token: &str) {
    let handle = handle.0;
    let mut data: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let mut read: u32 = 0;
        if unsafe { ReadFile(handle, Some(&mut buf), Some(&mut read), None) }.is_err() {
            break;
        }
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buf[..read as usize]);
        if data.len() >= MAX_MESSAGE || data.contains(&b'\n') {
            data.truncate(MAX_MESSAGE);
            break;
        }
    }

    unsafe {
        let _ = DisconnectNamedPipe(handle);
        let _ = CloseHandle(handle);
    }

    if data.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(&data);
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return;
    }

    match decode(line, token) {
        Ok(Some(message)) => {
            let _ = tx.send(message);
        }
        Ok(None) => {}
        Err(e) => {
            // A rejected frame means a hook was cast but not counted; leave a
            // trace so "the agent worked but Ember stayed idle" is diagnosable.
            crate::log::append("HOOK", &format!("rejected a hook message: {e}"));
        }
    }
}

/// Validate one wire frame and turn it into a registry message. Pure, so the
/// whole protocol surface is unit-testable.
pub fn decode(line: &str, token: &str) -> Result<Option<HookMessage>, String> {
    let wire: HookWire = serde_json::from_str(line).map_err(|e| format!("bad hook JSON: {e}"))?;
    if wire.v != PROTOCOL_VERSION {
        return Err(format!("unsupported hook protocol v{}", wire.v));
    }
    if wire.token != token {
        return Err("hook token mismatch".into());
    }
    let agent =
        AgentId::parse(&wire.agent).ok_or_else(|| format!("unknown agent {:?}", wire.agent))?;
    let event =
        HookEvent::parse(&wire.event).ok_or_else(|| format!("unknown event {:?}", wire.event))?;
    let ctx = parse_hook_payload(agent, &wire.payload);
    let pid = wire.pid.or(ctx.pid);
    Ok(Some(HookMessage {
        agent,
        event,
        session_id: ctx.session_id,
        cwd: ctx.cwd,
        pid,
        ts: wire.ts,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(token: &str, agent: &str, event: &str, payload: &str) -> String {
        serde_json::to_string(&HookWire {
            v: PROTOCOL_VERSION,
            token: token.to_string(),
            agent: agent.to_string(),
            event: event.to_string(),
            payload: payload.to_string(),
            pid: Some(4242),
            ts: 1_700_000_000,
        })
        .unwrap()
    }

    #[test]
    fn tokens_are_thirty_two_hex_chars_and_unique() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn a_well_formed_frame_decodes() {
        let line = frame(
            "tok",
            "claude-code",
            "working",
            r#"{"session_id":"abc","cwd":"C:\\work\\proj"}"#,
        );
        let msg = decode(&line, "tok").unwrap().unwrap();
        assert_eq!(msg.agent, AgentId::ClaudeCode);
        assert_eq!(msg.event, HookEvent::Working);
        assert_eq!(msg.session_id.as_deref(), Some("abc"));
        assert_eq!(msg.cwd.as_deref(), Some("C:\\work\\proj"));
        assert_eq!(msg.pid, Some(4242));
    }

    #[test]
    fn every_agent_and_event_round_trips() {
        for agent in AgentId::ALL {
            for event in [
                HookEvent::Start,
                HookEvent::Working,
                HookEvent::Idle,
                HookEvent::End,
                HookEvent::Heartbeat,
            ] {
                let line = frame("tok", agent.id(), event.as_str(), "");
                let msg = decode(&line, "tok").unwrap().unwrap();
                assert_eq!(msg.agent, agent);
                assert_eq!(msg.event, event);
            }
        }
    }

    #[test]
    fn a_wrong_token_is_rejected() {
        let line = frame("right", "codex", "idle", "");
        assert!(decode(&line, "wrong").is_err());
        assert!(decode(&line, "").is_err());
    }

    #[test]
    fn unknown_agents_events_and_versions_are_rejected() {
        assert!(decode(&frame("t", "not-an-agent", "idle", ""), "t").is_err());
        assert!(decode(&frame("t", "codex", "not-an-event", ""), "t").is_err());
        let mut wire: serde_json::Value =
            serde_json::from_str(&frame("t", "codex", "idle", "")).unwrap();
        wire["v"] = serde_json::json!(99);
        let err = decode(&wire.to_string(), "t").unwrap_err();
        assert!(err.contains("protocol"));
    }

    #[test]
    fn malformed_json_never_panics() {
        for line in ["", "{", "null", "[]", "\"hello\"", "{}"] {
            assert!(decode(line, "t").is_err(), "input {line:?}");
        }
    }

    #[test]
    fn missing_optional_fields_are_tolerated() {
        let line = r#"{"v":1,"token":"t","agent":"pi","event":"heartbeat"}"#;
        let msg = decode(line, "t").unwrap().unwrap();
        assert_eq!(msg.session_id, None);
        assert_eq!(msg.cwd, None);
        assert_eq!(msg.pid, None);
    }

    #[test]
    fn a_frame_wrapper_is_a_single_json_line() {
        let line = frame("t", "cline", "end", "not json at all");
        assert!(!line.contains('\n'));
        let msg = decode(&line, "t").unwrap().unwrap();
        assert_eq!(msg.event, HookEvent::End);
        assert_eq!(msg.session_id, None, "unparseable payload degrades safely");
    }
}
