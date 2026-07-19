use chrono::Local;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::execute;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::backtrace::Backtrace;
use std::collections::{HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::panic;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::SystemTime;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

const RING_BUF_CAPACITY: usize = 1000;

struct RingBuffer {
    buffer: VecDeque<String>,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        RingBuffer {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, entry: String) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(entry);
    }

    fn drain(&mut self) -> Vec<String> {
        self.buffer.drain(..).collect()
    }
}

fn ring_buffer() -> &'static Mutex<RingBuffer> {
    static RB: OnceLock<Mutex<RingBuffer>> = OnceLock::new();
    RB.get_or_init(|| Mutex::new(RingBuffer::new(RING_BUF_CAPACITY)))
}

pub struct QstLogger {
    file: Mutex<File>,
    level: LevelFilter,
}

impl QstLogger {
    pub fn initialize(level: LevelFilter, retention_days: Option<u64>) -> Result<(), SetLoggerError> {
        let log_path = get_log_path();
        let _ = LOG_PATH.set(log_path.clone());
        let sessions_dir = get_sessions_dir();

        if let Some(parent) = log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if log_path.exists() {
            let _ = fs::create_dir_all(&sessions_dir);
            let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
            let session_path = sessions_dir.join(format!("{}.log", timestamp));
            let _ = fs::rename(&log_path, &session_path);
        }

        cleanup_old_sessions(retention_days);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .expect("Failed to open log file");

        let logger = QstLogger {
            file: Mutex::new(file),
            level,
        };

        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(level);
        Ok(())
    }
}

impl Log for QstLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level = record.level();
        let file = record.file().unwrap_or("<unknown>");
        let line = record.line().unwrap_or(0);
        let args = record.args();

        let log_line =
            format!("[{}] [{}] [{}:{}] {}\n", timestamp, level, file, line, args);

        if let Ok(mut ring) = ring_buffer().lock() {
            ring.push(log_line.clone());
        }

        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(log_line.as_bytes());
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn get_log_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".local/state/qst/qst.log")
}

fn get_sessions_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".local/state/qst/sessions")
}

fn cleanup_old_sessions(retention_days: Option<u64>) {
    let Some(days) = retention_days.filter(|d| *d > 0) else {
        return;
    };

    let cutoff = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(days * 86400);

    let sessions_dir = get_sessions_dir();
    if let Ok(entries) = fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "log") {
                continue;
            }
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(mtime) = metadata.modified() {
                    if let Ok(duration) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
                        if duration.as_secs() < cutoff {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }

    let qst_dir = get_plugin_log_dir();
    if let Ok(entries) = fs::read_dir(&qst_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().map_or(false, |n| n.to_string_lossy().ends_with("_sessions")) {
                if let Ok(dir_entries) = fs::read_dir(&path) {
                    for file_entry in dir_entries.flatten() {
                        let file_path = file_entry.path();
                        if file_path.extension().map_or(true, |e| e != "log") {
                            continue;
                        }
                        if let Ok(metadata) = fs::metadata(&file_path) {
                            if let Ok(mtime) = metadata.modified() {
                                if let Ok(duration) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
                                    if duration.as_secs() < cutoff {
                                        let _ = fs::remove_file(&file_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn parse_log_level(value: &str) -> LevelFilter {
    match value.to_lowercase().as_str() {
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" | "warning" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

pub fn install_panic_hook() {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "<unknown>".to_string());

        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", panic_info.payload())
        };

        let backtrace = Backtrace::capture();
        let details = format!(
            "[{}] [PANIC] [{}] {}\nBacktrace:\n{}\n",
            timestamp, location, msg, backtrace,
        );

        if let Some(path) = LOG_PATH.get() {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = file.write_all(details.as_bytes());
                let _ = file.flush();
            }

            if let Some(crash_dir) = path.parent() {
                let crash_path = crash_dir.join("crash.log");
                if let Ok(mut crash) = OpenOptions::new().create(true).append(true).open(&crash_path) {
                    let _ = crash.write_all(details.as_bytes());
                    let _ = crash.write_all(b"--- Flight Recorder ---\n");
                    if let Ok(mut ring) = ring_buffer().lock() {
                        for entry in ring.drain() {
                            let _ = crash.write_all(entry.as_bytes());
                        }
                    }
                    let _ = crash.flush();
                }
            }
        }

        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        let _ = io::stderr().write_all(details.as_bytes());

        prev_hook(panic_info);
    }));
}

fn rotated_plugins() -> &'static Mutex<HashSet<String>> {
    static PLUGINS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PLUGINS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn get_plugin_log_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".local/state/qst")
}

fn get_plugin_log_path(plugin_id: &str) -> PathBuf {
    get_plugin_log_dir().join(format!("{}.log", plugin_id))
}

fn get_plugin_sessions_dir(plugin_id: &str) -> PathBuf {
    get_plugin_log_dir().join(format!("{}_sessions", plugin_id))
}

fn rotate_plugin_log(plugin_id: &str) {
    let log_path = get_plugin_log_path(plugin_id);
    if !log_path.exists() {
        return;
    }

    let sessions_dir = get_plugin_sessions_dir(plugin_id);
    let _ = fs::create_dir_all(&sessions_dir);
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let session_path = sessions_dir.join(format!("{}.log", timestamp));
    let _ = fs::rename(&log_path, &session_path);
}

/// Used by tests to reset plugin rotation state.
#[doc(hidden)]
pub fn __test_reset_plugin_rotation() {
    if let Ok(mut rotated) = rotated_plugins().lock() {
        rotated.clear();
    }
}

pub fn plugin_log(plugin_id: &str, message: &str) {
    {
        let mut rotated = rotated_plugins().lock().expect("plugin rotation lock");
        if rotated.insert(plugin_id.to_string()) {
            rotate_plugin_log(plugin_id);
        }
    }

    let log_path = get_plugin_log_path(plugin_id);
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let log_line = format!("[{}] {}\n", timestamp, message);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(log_line.as_bytes());
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_temp_home(test: fn(PathBuf)) {
        let _guard = TEST_LOCK.lock().unwrap();
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "qst-log-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        let original_home = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", dir.to_str().unwrap()); }
        __test_reset_plugin_rotation();
        test(dir.clone());
        let _ = fs::remove_dir_all(&dir);
        if let Some(home) = original_home {
            unsafe { std::env::set_var("HOME", home); }
        }
    }

    #[test]
    fn plugin_log_creates_file_with_timestamped_message() {
        with_temp_home(|dir| {
            let plugin_id = "test_logger";
            plugin_log(plugin_id, "hello from plugin");
            plugin_log(plugin_id, "second message");

            let log_path = dir.join(".local/state/qst").join(format!("{}.log", plugin_id));
            let content = fs::read_to_string(&log_path).unwrap_or_else(|_| String::new());

            assert!(content.contains("hello from plugin"), "log should contain first message");
            assert!(content.contains("second message"), "log should contain second message");
            assert!(content.starts_with('['), "log should start with timestamp bracket");
        });
    }

    #[test]
    fn plugin_log_archives_previous_session() {
        with_temp_home(|dir| {
            let _ = fs::create_dir_all(dir.join(".local/state/qst"));

            let log_path = dir.join(".local/state/qst/test_logger.log");
            fs::write(&log_path, "old data\n").unwrap();

            plugin_log("test_logger", "new session message");

            let sessions_dir = dir.join(".local/state/qst/test_logger_sessions");
            assert!(sessions_dir.exists(), "sessions dir should exist");
            let entries: Vec<_> = fs::read_dir(&sessions_dir).unwrap().collect();
            assert_eq!(entries.len(), 1, "one archived session should exist");

            let content = fs::read_to_string(&log_path).unwrap();
            assert!(content.contains("new session message"), "new log should have new message");
            assert!(!content.contains("old data"), "new log should not have old data");
        });
    }
}
