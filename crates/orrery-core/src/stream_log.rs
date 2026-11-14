use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::security::SecretRedactor;
use crate::{Error, Result};

pub const LOG_FILE: &str = "live_stream.log";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEventKind {
    Output,
    Reasoning,
    SystemNotice,
    ToolCall,
    ReportSaved,
    GateResult,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamEvent {
    pub timestamp: DateTime<Utc>,
    pub step_number: Option<usize>,
    pub stage_index: Option<usize>,
    pub agent_id: Option<String>,
    pub kind: StreamEventKind,
    pub text: String,
}

impl StreamEvent {
    pub fn notice(text: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            step_number: None,
            stage_index: None,
            agent_id: None,
            kind: StreamEventKind::SystemNotice,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveStreamConfig {
    pub ring_capacity: usize,
    pub max_file_bytes: u64,
    pub max_files: usize,
}

impl Default for LiveStreamConfig {
    fn default() -> Self {
        Self {
            ring_capacity: 2_000,
            max_file_bytes: 10 * 1024 * 1024,
            max_files: 3,
        }
    }
}

pub struct LiveStreamHub {
    log_dir: PathBuf,
    config: LiveStreamConfig,
    redactor: Arc<SecretRedactor>,
    state: Mutex<(VecDeque<StreamEvent>, u64)>,
    sender: broadcast::Sender<StreamEvent>,
}

impl LiveStreamHub {
    pub fn new(
        log_dir: PathBuf,
        config: LiveStreamConfig,
        redactor: Arc<SecretRedactor>,
    ) -> Result<Self> {
        fs::create_dir_all(&log_dir).map_err(|source| Error::Io {
            path: log_dir.clone(),
            source,
        })?;

        let log_path = log_dir.join(LOG_FILE);
        let current_size = match fs::metadata(&log_path) {
            Ok(meta) => meta.len(),
            Err(err) if err.kind() == io::ErrorKind::NotFound => 0,
            Err(source) => {
                return Err(Error::Io {
                    path: log_path,
                    source,
                });
            }
        };

        let channel_capacity = config.ring_capacity.max(1);
        let (sender, _) = broadcast::channel(channel_capacity);

        Ok(Self {
            log_dir,
            config,
            redactor,
            state: Mutex::new((VecDeque::with_capacity(config.ring_capacity), current_size)),
            sender,
        })
    }

    pub fn emit(&self, mut event: StreamEvent) -> Result<()> {
        event.text = self.redactor.redact(&event.text)?;

        {
            let mut guard = self.state.lock().map_err(|_| Error::Security {
                message: "LiveStreamHub lock poisoned".into(),
            })?;

            if self.config.ring_capacity > 0 {
                if guard.0.len() >= self.config.ring_capacity {
                    guard.0.pop_front();
                }
                guard.0.push_back(event.clone());
            }

            if self.config.max_files > 0
                && let Some(line) = fit_event_line(&event, self.config.max_file_bytes)?
            {
                let line_len = line.len() as u64;
                if guard.1 + line_len > self.config.max_file_bytes {
                    self.rotate_files()?;
                    guard.1 = 0;
                }
                let log_path = self.log_dir.join(LOG_FILE);
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .map_err(|source| Error::Io {
                        path: log_path.clone(),
                        source,
                    })?;
                file.write_all(line.as_bytes())
                    .map_err(|source| Error::Io {
                        path: log_path.clone(),
                        source,
                    })?;
                file.flush().map_err(|source| Error::Io {
                    path: log_path,
                    source,
                })?;
                guard.1 += line_len;
            }
        }

        let _ = self.sender.send(event);
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<StreamEvent>> {
        let guard = self.state.lock().map_err(|_| Error::Security {
            message: "LiveStreamHub lock poisoned".into(),
        })?;
        let skip = guard.0.len().saturating_sub(limit);
        Ok(guard.0.iter().skip(skip).cloned().collect())
    }

    pub fn redactor(&self) -> &Arc<SecretRedactor> {
        &self.redactor
    }

    fn rotate_files(&self) -> Result<()> {
        if self.config.max_files <= 1 {
            let log_path = self.log_dir.join(LOG_FILE);
            remove_if_exists(&log_path)?;
            return Ok(());
        }

        let oldest = rotated_log_path(&self.log_dir, self.config.max_files - 1);
        remove_if_exists(&oldest)?;

        for k in (1..=self.config.max_files - 2).rev() {
            let src = rotated_log_path(&self.log_dir, k);
            let dst = rotated_log_path(&self.log_dir, k + 1);
            rename_if_exists(&src, &dst)?;
        }

        let base = self.log_dir.join(LOG_FILE);
        let first_rot = rotated_log_path(&self.log_dir, 1);
        rename_if_exists(&base, &first_rot)?;

        Ok(())
    }
}

pub fn tail_log(log_dir: &Path, limit: usize) -> Result<Vec<String>> {
    let candidate_paths = [
        rotated_log_path(log_dir, 2),
        rotated_log_path(log_dir, 1),
        log_dir.join(LOG_FILE),
    ];

    let mut lines = Vec::new();
    for path in candidate_paths {
        match fs::read_to_string(&path) {
            Ok(content) => {
                lines.extend(content.lines().map(str::to_owned));
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::Io { path, source }),
        }
    }

    let skip = lines.len().saturating_sub(limit);
    Ok(lines.into_iter().skip(skip).collect())
}

fn rotated_log_path(log_dir: &Path, index: usize) -> PathBuf {
    log_dir.join(format!("{LOG_FILE}.{index}"))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn rename_if_exists(src: &Path, dst: &Path) -> Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: src.to_path_buf(),
            source,
        }),
    }
}

fn serialize_event_line(event: &StreamEvent) -> Result<String> {
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    Ok(line)
}

fn fit_event_line(event: &StreamEvent, max_file_bytes: u64) -> Result<Option<String>> {
    let full_line = serialize_event_line(event)?;
    if full_line.len() as u64 <= max_file_bytes {
        return Ok(Some(full_line));
    }

    let mut shortened = event.clone();
    shortened.text.clear();
    let empty_line = serialize_event_line(&shortened)?;
    if empty_line.len() as u64 > max_file_bytes {
        return Ok(None);
    }

    let boundaries: Vec<usize> = event
        .text
        .char_indices()
        .map(|(idx, _)| idx)
        .chain(std::iter::once(event.text.len()))
        .collect();

    let mut low = 0usize;
    let mut high = boundaries.len() - 1;
    let mut best_line = empty_line;

    while low <= high {
        let mid = low + (high - low) / 2;
        let byte_end = boundaries[mid];
        shortened.text.clear();
        shortened.text.push_str(&event.text[..byte_end]);
        let candidate = serialize_event_line(&shortened)?;
        if candidate.len() as u64 <= max_file_bytes {
            best_line = candidate;
            low = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            high = mid - 1;
        }
    }

    Ok(Some(best_line))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use chrono::Utc;
    use tempfile::TempDir;

    use super::{
        LOG_FILE, LiveStreamConfig, LiveStreamHub, StreamEvent, StreamEventKind, rotated_log_path,
        tail_log,
    };
    use crate::security::{REDACTED, SecretRedactor, SecretValue};

    fn test_config() -> LiveStreamConfig {
        LiveStreamConfig {
            ring_capacity: 4,
            max_file_bytes: 512,
            max_files: 3,
        }
    }

    fn make_hub(dir: &TempDir) -> LiveStreamHub {
        let redactor = Arc::new(SecretRedactor::new());
        LiveStreamHub::new(dir.path().to_path_buf(), test_config(), redactor)
            .expect("create LiveStreamHub")
    }

    #[test]
    fn ring_evicts_oldest() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        for i in 1..=6 {
            hub.emit(StreamEvent::notice(format!("event-{i}")))
                .expect("emit event");
        }

        let recent = hub.recent(10).expect("read recent");
        let texts: Vec<&str> = recent.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, vec!["event-3", "event-4", "event-5", "event-6"]);

        let last_two = hub.recent(2).expect("read recent 2");
        let last_two_texts: Vec<&str> = last_two.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(last_two_texts, vec!["event-5", "event-6"]);
    }

    #[test]
    fn secret_redacted_in_ring_and_file() {
        let dir = TempDir::new().expect("tempdir");
        let redactor = Arc::new(SecretRedactor::new());
        let raw_secret = "super-secret-value-999";
        redactor
            .register(&SecretValue::new(raw_secret))
            .expect("register secret");

        let hub = LiveStreamHub::new(dir.path().to_path_buf(), test_config(), redactor)
            .expect("create hub");
        assert!(Arc::ptr_eq(hub.redactor(), &hub.redactor));

        for i in 0..10 {
            hub.emit(StreamEvent {
                timestamp: Utc::now(),
                step_number: Some(1),
                stage_index: Some(0),
                agent_id: Some("coder".into()),
                kind: StreamEventKind::Output,
                text: format!("msg {i}: key={raw_secret}"),
            })
            .expect("emit secret event");
        }

        let recent = hub.recent(10).expect("recent");
        assert!(!recent.is_empty());
        for ev in &recent {
            assert!(!ev.text.contains(raw_secret));
            assert!(ev.text.contains(REDACTED));
        }

        for path in [
            dir.path().join(LOG_FILE),
            rotated_log_path(dir.path(), 1),
            rotated_log_path(dir.path(), 2),
        ] {
            if path.exists() {
                let content = fs::read_to_string(&path).expect("read log file");
                assert!(!content.contains(raw_secret));
                assert!(content.contains(REDACTED));
            }
        }
    }

    #[test]
    fn rotation_order() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        for i in 1..=20 {
            hub.emit(StreamEvent::notice(format!(
                "rotation-entry-{i:02}-padding-text"
            )))
            .expect("emit");
        }

        let log0 = dir.path().join(LOG_FILE);
        let log1 = rotated_log_path(dir.path(), 1);
        let log2 = rotated_log_path(dir.path(), 2);

        assert!(log0.exists());
        assert!(log1.exists());
        assert!(log2.exists());

        let parse_first_seq = |path: &std::path::Path| -> usize {
            let content = fs::read_to_string(path).expect("read rotated log");
            let first_line = content.lines().next().expect("at least one line");
            let ev: StreamEvent = serde_json::from_str(first_line).expect("parse StreamEvent");
            ev.text
                .strip_prefix("rotation-entry-")
                .and_then(|s| s.split('-').next())
                .and_then(|n| n.parse::<usize>().ok())
                .expect("sequence number")
        };

        let seq2 = parse_first_seq(&log2);
        let seq1 = parse_first_seq(&log1);
        let seq0 = parse_first_seq(&log0);
        assert!(seq2 < seq1, "expected .log.2 ({seq2}) < .log.1 ({seq1})");
        assert!(seq1 < seq0, "expected .log.1 ({seq1}) < .log ({seq0})");
    }

    #[test]
    fn oldest_file_dropped() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        for i in 1..=40 {
            hub.emit(StreamEvent::notice(format!(
                "drop-test-{i:02}-extra-payload"
            )))
            .expect("emit");
        }

        assert!(dir.path().join(LOG_FILE).exists());
        assert!(rotated_log_path(dir.path(), 1).exists());
        assert!(rotated_log_path(dir.path(), 2).exists());
        assert!(!rotated_log_path(dir.path(), 3).exists());
    }

    #[test]
    fn total_bytes_never_exceed_cap() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);
        let max_total = 512u64 * 3;

        for i in 0..1_000 {
            hub.emit(StreamEvent::notice(format!("event-number-{i}-payload")))
                .expect("emit");

            let mut total = 0u64;
            for path in [
                dir.path().join(LOG_FILE),
                rotated_log_path(dir.path(), 1),
                rotated_log_path(dir.path(), 2),
            ] {
                if let Ok(meta) = fs::metadata(&path) {
                    assert!(meta.len() <= 512, "single file exceeded 512 bytes");
                    total += meta.len();
                }
            }
            assert!(
                total <= max_total,
                "total bytes {total} exceeded cap {max_total} at emit {i}"
            );
            assert!(!rotated_log_path(dir.path(), 3).exists());
        }
    }

    #[test]
    fn oversize_line_is_shortened() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        let long_text = "é".repeat(2_000);
        hub.emit(StreamEvent::notice(long_text))
            .expect("emit oversize event");

        let log_path = dir.path().join(LOG_FILE);
        let raw = fs::read(&log_path).expect("read log file");
        assert!(
            raw.len() <= 512,
            "expected written line <= 512 bytes, got {}",
            raw.len()
        );
        assert_eq!(raw.last().copied(), Some(b'\n'));

        let parsed: StreamEvent =
            serde_json::from_slice(&raw).expect("shortened line is valid StreamEvent JSON");
        assert!(!parsed.text.is_empty());
        assert!(parsed.text.chars().all(|c| c == 'é'));
    }

    #[test]
    fn subscriber_receives_redacted_event() {
        let dir = TempDir::new().expect("tempdir");
        let redactor = Arc::new(SecretRedactor::new());
        redactor
            .register(&SecretValue::new("secret-token-abcdef"))
            .expect("register");
        let hub = LiveStreamHub::new(dir.path().to_path_buf(), test_config(), redactor)
            .expect("create hub");

        let mut rx = hub.subscribe();
        hub.emit(StreamEvent::notice("using secret-token-abcdef now"))
            .expect("emit");

        let received = rx.try_recv().expect("receive broadcast event");
        assert_eq!(received.text, "using [REDACTED] now");
        assert_eq!(received.kind, StreamEventKind::SystemNotice);
    }

    #[test]
    fn emit_without_subscribers_is_ok() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        hub.emit(StreamEvent::notice("no subscribers listening"))
            .expect("emit without subscribers should succeed");
        let recent = hub.recent(1).expect("recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].text, "no subscribers listening");
    }

    #[test]
    fn tail_log_reads_across_rotated_files() {
        let dir = TempDir::new().expect("tempdir");
        let hub = make_hub(&dir);

        for i in 1..=18 {
            hub.emit(StreamEvent::notice(format!(
                "tail-item-{i:02}-padding-value"
            )))
            .expect("emit");
        }

        assert!(rotated_log_path(dir.path(), 2).exists());
        assert!(rotated_log_path(dir.path(), 1).exists());
        assert!(dir.path().join(LOG_FILE).exists());

        let all_lines = tail_log(dir.path(), 100).expect("tail all");
        assert!(all_lines.len() >= 5);

        let tail_five = tail_log(dir.path(), 5).expect("tail 5");
        assert_eq!(tail_five.len(), 5);

        let parsed_texts: Vec<String> = tail_five
            .iter()
            .map(|line| {
                let ev: StreamEvent = serde_json::from_str(line).expect("valid JSON line");
                ev.text
            })
            .collect();

        let expected: Vec<String> = (14..=18)
            .map(|i| format!("tail-item-{i:02}-padding-value"))
            .collect();
        assert_eq!(parsed_texts, expected);
    }

    #[test]
    fn default_config_and_unfit_empty_line_and_existing_file_size() {
        let def = LiveStreamConfig::default();
        assert_eq!(def.ring_capacity, 2_000);
        assert_eq!(def.max_file_bytes, 10 * 1024 * 1024);
        assert_eq!(def.max_files, 3);

        let dir = TempDir::new().expect("tempdir");
        let tiny_cfg = LiveStreamConfig {
            ring_capacity: 2,
            max_file_bytes: 10,
            max_files: 1,
        };
        let hub = LiveStreamHub::new(
            dir.path().to_path_buf(),
            tiny_cfg,
            Arc::new(SecretRedactor::new()),
        )
        .expect("hub");
        hub.emit(StreamEvent::notice(
            "skipped file write because max_file_bytes is 10",
        ))
        .expect("emit when empty event exceeds max_file_bytes");
        assert!(!dir.path().join(LOG_FILE).exists());
        assert_eq!(hub.recent(5).expect("recent").len(), 1);

        let dir2 = TempDir::new().expect("tempdir2");
        let hub1 = make_hub(&dir2);
        hub1.emit(StreamEvent::notice("initial-line"))
            .expect("emit initial");
        let hub2 = make_hub(&dir2);
        hub2.emit(StreamEvent::notice("second-line"))
            .expect("emit second");
        let tailed = tail_log(dir2.path(), 10).expect("tail");
        assert_eq!(tailed.len(), 2);
    }
}
