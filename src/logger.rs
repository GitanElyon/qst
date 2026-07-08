use chrono::Local;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct QstLogger {
    file: Mutex<File>,
    level: LevelFilter,
}

impl QstLogger {
    pub fn initialize(level: LevelFilter) -> Result<(), SetLoggerError> {
        let log_path = get_log_path();
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

pub fn parse_log_level(value: &str) -> LevelFilter {
    match value.to_lowercase().as_str() {
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" | "warning" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}
