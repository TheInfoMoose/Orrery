use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

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

enum WriterMsg {
    Line(String),
    Flush(mpsc::Sender<Option<String>>),
}

struct LogWriter {
    log_dir: PathBuf,
    max_file_bytes: u64,
    max_files: usize,
    current_size: u64,
    file: Option<File>,
    kept_error: Option<String>,
    shared_error: Arc<Mutex<Option<String>>>,
}

impl LogWriter {
    fn record_error(&mut self, path: &Path, err: &io::Error) {
        if self.kept_error.is_none() {
            let msg = format!("{}: {err}", path.display());
            self.kept_error = Some(msg.clone());
            if let Ok(mut slot) = self.shared_error.lock() {
                *slot = Some(msg.clone());
            }
        }
    }

    fn write_line(&mut self, line: &str) {
        let line_len = line.len() as u64;
        if self.current_size + line_len > self.max_file_bytes {
            self.file = None;
            if let Err((path, err)) = rotate_files(&self.log_dir, self.max_files) {
                self.record_error(&path, &err);
                return;
            }
            self.current_size = 0;
        }

        let log_path = self.log_dir.join(LOG_FILE);
        if self.file.is_none() {
            match OpenOptions::new().create(true).append(true).open(&log_path) {
                Ok(file) => self.file = Some(file),
                Err(err) => {
                    self.record_error(&log_path, &err);
                    return;
                }
            }
        }

        if let Some(file) = self.file.as_mut() {
            if let Err(err) = file.write_all(line.as_bytes()).and_then(|()| file.flush()) {
                self.file = None;
                self.record_error(&log_path, &err);
                return;
            }
            self.current_size += line_len;
        }
    }
}

pub struct LiveStreamHub {
    log_dir: PathBuf,
    config: LiveStreamConfig,
    redactor: Arc<SecretRedactor>,
    ring: Mutex<VecDeque<StreamEvent>>,
    writer_tx: Mutex<Option<mpsc::Sender<WriterMsg>>>,
    writer_handle: Option<JoinHandle<()>>,
    write_error: Arc<Mutex<Option<String>>>,
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

        let (tx, rx) = mpsc::channel::<WriterMsg>();
        let writer_log_dir = log_dir.clone();
        let max_file_bytes = config.max_file_bytes;
        let max_files = config.max_files;
        let write_error = Arc::new(Mutex::new(None));
        let shared_error = Arc::clone(&write_error);
        let writer_handle = thread::Builder::new()
            .name("orrery-log-writer".into())
            .spawn(move || {
                let mut writer = LogWriter {
                    log_dir: writer_log_dir,
                    max_file_bytes,
                    max_files,
                    current_size,
                    file: None,
                    kept_error: None,
                    shared_error,
                };
                while let Ok(msg) = rx.recv() {
                    match msg {
                        WriterMsg::Line(line) => writer.write_line(&line),
                        WriterMsg::Flush(reply) => {
                            let _ = reply.send(writer.kept_error.take());
                        }
                    }
                }
            })
            .map_err(|source| Error::Io {
                path: log_dir.clone(),
                source,
            })?;

        let channel_capacity = config.ring_capacity.max(1);
        let (sender, _) = broadcast::channel(channel_capacity);

        Ok(Self {
            log_dir,
            config,
            redactor,
            ring: Mutex::new(VecDeque::with_capacity(config.ring_capacity)),
            writer_tx: Mutex::new(Some(tx)),
            writer_handle: Some(writer_handle),
            write_error,
            sender,
        })
    }

    pub fn emit(&self, mut event: StreamEvent) -> Result<()> {
        event.text = self.redactor.redact(&event.text)?;

        {
            let mut ring = self.ring.lock().map_err(|_| Error::Security {
                message: "LiveStreamHub lock poisoned".into(),
            })?;

            if self.config.ring_capacity > 0 {
                if ring.len() >= self.config.ring_capacity {
                    ring.pop_front();
                }
                ring.push_back(event.clone());
            }
        }

        if self.config.max_files > 0
            && let Some(line) = fit_event_line(&event, self.config.max_file_bytes)?
        {
            let guard = self.writer_tx.lock().map_err(|_| Error::Security {
                message: "LiveStreamHub lock poisoned".into(),
            })?;
            let tx = guard.as_ref().ok_or_else(|| Error::Security {
                message: "LiveStreamHub writer stopped".into(),
            })?;
            tx.send(WriterMsg::Line(line))
                .map_err(|_| Error::Security {
                    message: "LiveStreamHub writer stopped".into(),
                })?;
        }

        let _ = self.sender.send(event);

        let write_err = {
            let mut slot = self.write_error.lock().map_err(|_| Error::Security {
                message: "LiveStreamHub lock poisoned".into(),
            })?;
            slot.take()
        };

        if let Some(msg) = write_err {
            let err_event = StreamEvent {
                timestamp: Utc::now(),
                step_number: None,
                stage_index: None,
                agent_id: None,
                kind: StreamEventKind::Error,
                text: self
                    .redactor
                    .redact(&format!("live stream log write failed: {msg}"))?,
            };
            {
                let mut ring = self.ring.lock().map_err(|_| Error::Security {
                    message: "LiveStreamHub lock poisoned".into(),
                })?;
                if self.config.ring_capacity > 0 {
                    if ring.len() >= self.config.ring_capacity {
                        ring.pop_front();
                    }
                    ring.push_back(err_event.clone());
                }
            }
            let _ = self.sender.send(err_event);
        }

        Ok(())
    }

    pub fn flush(&self) -> Result<()> {
        let (reply_tx, reply_rx) = mpsc::channel();
        {
            let guard = self.writer_tx.lock().map_err(|_| Error::Security {
                message: "LiveStreamHub lock poisoned".into(),
            })?;
            let tx = guard.as_ref().ok_or_else(|| Error::Security {
                message: "LiveStreamHub writer stopped".into(),
            })?;
            tx.send(WriterMsg::Flush(reply_tx))
                .map_err(|_| Error::Security {
                    message: "LiveStreamHub writer stopped".into(),
                })?;
        }

        match reply_rx.recv() {
            Ok(None) => Ok(()),
            Ok(Some(msg)) => Err(Error::Io {
                path: self.log_dir.join(LOG_FILE),
                source: io::Error::other(msg),
            }),
            Err(_) => Err(Error::Security {
                message: "LiveStreamHub writer stopped".into(),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<StreamEvent>> {
        let ring = self.ring.lock().map_err(|_| Error::Security {
            message: "LiveStreamHub lock poisoned".into(),
        })?;
        let skip = ring.len().saturating_sub(limit);
        Ok(ring.iter().skip(skip).cloned().collect())
    }

    pub fn redactor(&self) -> &Arc<SecretRedactor> {
        &self.redactor
    }
}

impl Drop for LiveStreamHub {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.writer_tx.lock() {
            let _ = guard.take();
        }
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.join();
        }
    }
}

fn rotate_files(log_dir: &Path, max_files: usize) -> std::result::Result<(), (PathBuf, io::Error)> {
    if max_files <= 1 {
        let log_path = log_dir.join(LOG_FILE);
        remove_if_exists(&log_path)?;
        return Ok(());
    }

    let oldest = rotated_log_path(log_dir, max_files - 1);
    remove_if_exists(&oldest)?;

    for k in (1..=max_files - 2).rev() {
        let src = rotated_log_path(log_dir, k);
        let dst = rotated_log_path(log_dir, k + 1);
        rename_if_exists(&src, &dst)?;
    }

    let base = log_dir.join(LOG_FILE);
    let first_rot = rotated_log_path(log_dir, 1);
    rename_if_exists(&base, &first_rot)?;

    Ok(())
}

pub fn tail_log(log_dir: &Path, limit: usize) -> Result<Vec<String>> {
    let candidate_paths = (1..=9)
        .rev()
        .map(|i| rotated_log_path(log_dir, i))
        .chain(std::iter::once(log_dir.join(LOG_FILE)));

    let mut lines = Vec::new();
    for path in candidate_paths {
        match fs::read(&path) {
            Ok(bytes) => {
                lines.extend(String::from_utf8_lossy(&bytes).lines().map(str::to_owned));
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::Io { path, source }),
        }
    }

    let skip = lines.len().saturating_sub(limit);
    Ok(lines.into_iter().skip(skip).collect())
}

pub struct LogFollower {
    path: PathBuf,
    file: Option<File>,
    offset: u64,
    ino: u64,
    partial: Vec<u8>,
}

impl LogFollower {
    pub fn new(log_dir: &Path) -> Result<Self> {
        let path = log_dir.join(LOG_FILE);
        match File::open(&path) {
            Ok(mut file) => {
                let meta = file.metadata().map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?;
                let offset = file.seek(SeekFrom::End(0)).map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?;
                Ok(Self {
                    path,
                    file: Some(file),
                    offset,
                    ino: meta.ino(),
                    partial: Vec::new(),
                })
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self {
                path,
                file: None,
                offset: 0,
                ino: 0,
                partial: Vec::new(),
            }),
            Err(source) => Err(Error::Io { path, source }),
        }
    }

    pub fn poll(&mut self) -> Result<Vec<String>> {
        if let Some(file) = self.file.as_mut() {
            file.seek(SeekFrom::Start(self.offset))
                .map_err(|source| Error::Io {
                    path: self.path.clone(),
                    source,
                })?;
            let read_bytes = file
                .read_to_end(&mut self.partial)
                .map_err(|source| Error::Io {
                    path: self.path.clone(),
                    source,
                })?;
            self.offset += read_bytes as u64;
        }

        match fs::metadata(&self.path) {
            Ok(meta) => {
                if self.file.is_none() || meta.ino() != self.ino || meta.len() < self.offset {
                    match File::open(&self.path) {
                        Ok(mut file) => {
                            let open_meta = file.metadata().map_err(|source| Error::Io {
                                path: self.path.clone(),
                                source,
                            })?;
                            self.ino = open_meta.ino();
                            self.offset = 0;
                            let read_bytes =
                                file.read_to_end(&mut self.partial).map_err(|source| {
                                    Error::Io {
                                        path: self.path.clone(),
                                        source,
                                    }
                                })?;
                            self.offset = read_bytes as u64;
                            self.file = Some(file);
                        }
                        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                        Err(source) => {
                            return Err(Error::Io {
                                path: self.path.clone(),
                                source,
                            });
                        }
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    path: self.path.clone(),
                    source,
                });
            }
        }

        let mut lines = Vec::new();
        while let Some(pos) = self.partial.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.partial.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&raw[..pos])
                .trim_end_matches('\r')
                .to_string();
            lines.push(line);
        }
        Ok(lines)
    }
}

fn rotated_log_path(log_dir: &Path, index: usize) -> PathBuf {
    log_dir.join(format!("{LOG_FILE}.{index}"))
}

fn remove_if_exists(path: &Path) -> std::result::Result<(), (PathBuf, io::Error)> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err((path.to_path_buf(), source)),
    }
}

fn rename_if_exists(src: &Path, dst: &Path) -> std::result::Result<(), (PathBuf, io::Error)> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err((src.to_path_buf(), source)),
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
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::sync::Arc;

    use chrono::Utc;
    use tempfile::TempDir;

    use super::{
        LOG_FILE, LiveStreamConfig, LiveStreamHub, LogFollower, StreamEvent, StreamEventKind,
        rotated_log_path, tail_log,
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

        hub.flush().unwrap();
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
        hub.flush().unwrap();

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
        hub.flush().unwrap();

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
            hub.flush().unwrap();

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
        hub.flush().unwrap();

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
        hub.flush().unwrap();

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
        hub.flush().unwrap();
        assert!(!dir.path().join(LOG_FILE).exists());
        assert_eq!(hub.recent(5).expect("recent").len(), 1);

        let dir2 = TempDir::new().expect("tempdir2");
        let hub1 = make_hub(&dir2);
        hub1.emit(StreamEvent::notice("initial-line"))
            .expect("emit initial");
        hub1.flush().unwrap();
        let hub2 = make_hub(&dir2);
        hub2.emit(StreamEvent::notice("second-line"))
            .expect("emit second");
        hub2.flush().unwrap();
        let tailed = tail_log(dir2.path(), 10).expect("tail");
        assert_eq!(tailed.len(), 2);
    }

    #[test]
    fn drop_writes_pending_lines() {
        let dir = TempDir::new().expect("tempdir");
        let hub = LiveStreamHub::new(
            dir.path().to_path_buf(),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .expect("create hub");

        for i in 1..=100 {
            hub.emit(StreamEvent::notice(format!("pending-line-{i}")))
                .expect("emit");
        }
        drop(hub);

        let lines = tail_log(dir.path(), usize::MAX).expect("tail_log");
        assert_eq!(lines.len(), 100);
    }

    #[test]
    fn flush_reports_write_error() {
        let dir = TempDir::new().expect("tempdir");
        let hub = LiveStreamHub::new(
            dir.path().to_path_buf(),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .expect("create hub");
        fs::create_dir(dir.path().join(LOG_FILE)).expect("create blocking directory");

        hub.emit(StreamEvent::notice("cannot write to directory"))
            .expect("emit should return Ok");
        let err = hub.flush().expect_err("first flush should return Err");
        assert!(matches!(err, crate::Error::Io { .. }));

        hub.flush()
            .expect("second flush should return Ok after error was taken");
    }

    #[test]
    fn rotation_unchanged_with_writer_thread() {
        let dir = TempDir::new().expect("tempdir");
        let cfg = LiveStreamConfig {
            ring_capacity: 64,
            max_file_bytes: 300,
            max_files: 3,
        };
        let hub = LiveStreamHub::new(
            dir.path().to_path_buf(),
            cfg,
            Arc::new(SecretRedactor::new()),
        )
        .expect("create hub");

        for i in 1..=30 {
            hub.emit(StreamEvent::notice(format!("writer-rot-{i:02}-payload")))
                .expect("emit");
        }
        hub.flush().unwrap();

        let log0 = dir.path().join(LOG_FILE);
        let log1 = rotated_log_path(dir.path(), 1);
        let log2 = rotated_log_path(dir.path(), 2);
        let log3 = rotated_log_path(dir.path(), 3);

        assert!(log0.exists());
        assert!(log1.exists());
        assert!(log2.exists());
        assert!(!log3.exists());

        for path in [&log0, &log1, &log2] {
            let len = fs::metadata(path).expect("metadata").len();
            assert!(len <= 300, "{} had {len} bytes (> 300)", path.display());
        }
    }

    #[test]
    fn follower_reads_appended_lines() {
        let dir = TempDir::new().expect("tempdir");
        let log_path = dir.path().join(LOG_FILE);
        fs::write(&log_path, "a\n").expect("write a");

        let mut follower = LogFollower::new(dir.path()).expect("new follower");
        let mut f = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open log");
        f.write_all(b"b\nc\n").expect("append b, c");
        f.flush().expect("flush");

        assert_eq!(
            follower.poll().expect("first poll"),
            vec!["b".to_string(), "c".to_string()]
        );
        assert!(follower.poll().expect("second poll").is_empty());
    }

    #[test]
    fn follower_survives_rotation() {
        let dir = TempDir::new().expect("tempdir");
        let log_path = dir.path().join(LOG_FILE);
        fs::write(&log_path, "a\n").expect("write a");

        let mut follower = LogFollower::new(dir.path()).expect("new follower");
        let mut f = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open log");
        f.write_all(b"b\n").expect("append b");
        f.flush().expect("flush");
        drop(f);

        fs::rename(&log_path, dir.path().join("live_stream.log.1")).expect("rename");
        fs::write(&log_path, "c\n").expect("write c");

        assert_eq!(
            follower.poll().expect("poll across rotation"),
            vec!["b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn follower_keeps_partial_line() {
        let dir = TempDir::new().expect("tempdir");
        let log_path = dir.path().join(LOG_FILE);
        fs::write(&log_path, "").expect("write empty");

        let mut follower = LogFollower::new(dir.path()).expect("new follower");
        let mut f = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open log");
        f.write_all(b"par").expect("append par");
        f.flush().expect("flush");

        assert!(follower.poll().expect("poll partial").is_empty());

        f.write_all(b"tial\n").expect("append tial");
        f.flush().expect("flush");

        assert_eq!(
            follower.poll().expect("poll completed"),
            vec!["partial".to_string()]
        );
    }

    #[test]
    fn follower_handles_split_utf8() {
        let dir = TempDir::new().expect("tempdir");
        let log_path = dir.path().join(LOG_FILE);
        fs::write(&log_path, "").expect("write empty");

        let mut follower = LogFollower::new(dir.path()).expect("new follower");
        let mut f = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open log");
        f.write_all(b"first\nsplit \xC3")
            .expect("append first part");
        f.flush().expect("flush");

        assert_eq!(
            follower.poll().expect("poll partial utf8"),
            vec!["first".to_string()]
        );

        f.write_all(b"\xA9 done\n").expect("append second part");
        f.flush().expect("flush");

        assert_eq!(
            follower.poll().expect("poll completed utf8"),
            vec!["split é done".to_string()]
        );
    }

    #[test]
    fn write_error_broadcasts_once() {
        let dir = TempDir::new().expect("tempdir");
        let hub = LiveStreamHub::new(
            dir.path().to_path_buf(),
            LiveStreamConfig::default(),
            Arc::new(SecretRedactor::new()),
        )
        .expect("create hub");
        fs::create_dir(dir.path().join(LOG_FILE)).expect("create blocking directory");

        hub.emit(StreamEvent::notice("a")).expect("emit a");
        let err = hub.flush().expect_err("flush should return Err");
        assert!(matches!(err, crate::Error::Io { .. }));

        hub.emit(StreamEvent::notice("b")).expect("emit b");

        let recent = hub.recent(20).expect("recent");
        let errors: Vec<&StreamEvent> = recent
            .iter()
            .filter(|e| e.kind == StreamEventKind::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].text.starts_with("live stream log write failed: "),
            "unexpected error text: {}",
            errors[0].text
        );
    }

    #[test]
    fn tail_log_reads_high_rotations() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(rotated_log_path(dir.path(), 4), "a\n").expect("write .log.4");
        fs::write(rotated_log_path(dir.path(), 1), "b\n").expect("write .log.1");
        fs::write(dir.path().join(LOG_FILE), "c\n").expect("write .log");

        assert_eq!(
            tail_log(dir.path(), 10).expect("tail_log"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn tail_log_tolerates_invalid_utf8() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(dir.path().join(LOG_FILE), b"x\xffy\n").expect("write invalid utf8");

        assert_eq!(
            tail_log(dir.path(), 10).expect("tail_log"),
            vec!["x\u{FFFD}y".to_string()]
        );
    }
}
