use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 128 * 1024;
const KEEP_LOG_BYTES: usize = 64 * 1024;
const DEFAULT_RECENT_LINES: usize = 200;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log_path() -> PathBuf {
    crate::paths::config_dir().join("ember.log")
}

/// Append one line to the live log. A no-op under unit tests: engine tests
/// tick hundreds of times and must not pollute the user-visible "Recent
/// activity" card (the log's own tests use `append_to_path`).
pub fn append(category: &str, message: &str) {
    #[cfg(test)]
    let _ = (category, message);
    #[cfg(not(test))]
    {
        let _guard = match LOG_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let path = log_path();
        let _ = append_to_path(&path, category, message);
    }
}

pub fn append_to_path(path: &PathBuf, category: &str, message: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_BYTES
    {
        rotate_in_place(path);
    }

    let ts = format_timestamp_ms();
    let clean_msg = message.replace(['\r', '\n'], " ");
    let line = format!("[{ts}] [{category}] {clean_msg}\n");

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.flush()
}

fn rotate_in_place(path: &PathBuf) {
    if let Ok(bytes) = fs::read(path)
        && bytes.len() > KEEP_LOG_BYTES
    {
        let start = bytes.len() - KEEP_LOG_BYTES;
        // Align to next newline so we don't keep a partial line
        let slice = &bytes[start..];
        let aligned = match slice.iter().position(|&b| b == b'\n') {
            Some(idx) if idx + 1 < slice.len() => &slice[idx + 1..],
            _ => slice,
        };
        let _ = fs::write(path, aligned);
    }
}

pub fn read_recent(max_lines: usize) -> String {
    let _guard = match LOG_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let path = log_path();
    read_recent_from_path(&path, max_lines)
}

pub fn read_recent_from_path(path: &PathBuf, max_lines: usize) -> String {
    let limit = if max_lines == 0 {
        DEFAULT_RECENT_LINES
    } else {
        max_lines
    };
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(limit);
    lines[start..].join("\n")
}

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".into());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".into()
        };
        append("PANIC", &format!("panic at {location}: {payload}"));
        default_hook(info);
    }));
}

fn format_timestamp_ms() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    format!("{secs}.{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_log_writes_and_reads_recent_lines() {
        let dir = std::env::temp_dir().join(format!("ember-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("ember.log");

        for i in 1..=10 {
            append_to_path(&path, "TEST", &format!("event {i}")).unwrap();
        }

        let recent = read_recent_from_path(&path, 3);
        let lines: Vec<&str> = recent.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("event 8"));
        assert!(lines[1].contains("event 9"));
        assert!(lines[2].contains("event 10"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rolling_log_rotates_when_exceeding_max_bytes() {
        let dir = std::env::temp_dir().join(format!("ember-log-rot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("ember.log");

        let chunk = "x".repeat(1024);
        for i in 0..150 {
            append_to_path(&path, "ROT", &format!("{i} {chunk}")).unwrap();
        }

        let meta = fs::metadata(&path).unwrap();
        assert!(meta.len() <= MAX_LOG_BYTES);

        let _ = fs::remove_dir_all(&dir);
    }
}
