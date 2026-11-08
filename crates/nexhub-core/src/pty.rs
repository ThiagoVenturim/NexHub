//! PTY session management, spawning processes, and stream handling.

use crate::model::SpawnSpec;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("pty system error: {0}")]
    Pty(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("session not found: {0}")]
    NotFound(String),
}

/// Ring buffer storing recent output up to a fixed byte capacity.
#[derive(Debug, Clone)]
pub struct RingBuffer {
    capacity: usize,
    buffer: VecDeque<u8>,
}

impl RingBuffer {
    pub const DEFAULT_CAPACITY: usize = 256 * 1024; // 256 KiB

    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push_slice(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if data.len() >= self.capacity {
            self.buffer.clear();
            self.buffer.extend(&data[data.len() - self.capacity..]);
            return;
        }
        let overflow = (self.buffer.len() + data.len()).saturating_sub(self.capacity);
        if overflow > 0 {
            self.buffer.drain(0..overflow);
        }
        self.buffer.extend(data);
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.buffer.iter().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

/// Formats a prompt string for PTY delivery. Wraps multi-line input in bracketed paste
/// if requested.
pub fn format_prompt(text: &str, bracketed_paste: bool) -> Vec<u8> {
    if bracketed_paste && text.contains('\n') {
        format!("\x1b[200~{text}\x1b[201~\r").into_bytes()
    } else {
        format!("{text}\r").into_bytes()
    }
}

/// Active PTY session wrapping a spawned child process.
pub struct PtySession {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    output_tx: broadcast::Sender<Vec<u8>>,
    ring_buffer: Arc<Mutex<RingBuffer>>,
    _reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl PtySession {
    /// Spawns a process inside a real pseudo-terminal according to `SpawnSpec`.
    pub fn spawn(spec: SpawnSpec) -> Result<Self, PtyError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Pty(e.to_string()))?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Pty(e.to_string()))?;

        // Drop the slave end in this process so child holds the only handle
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Pty(e.to_string()))?;

        let (output_tx, _) = broadcast::channel(1024);
        let ring_buffer = Arc::new(Mutex::new(RingBuffer::default()));

        let tx_clone = output_tx.clone();
        let ring_clone = Arc::clone(&ring_buffer);

        let reader_thread = std::thread::Builder::new()
            .name("pty-reader".to_string())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = buf[..n].to_vec();
                            ring_clone.lock().unwrap().push_slice(&chunk);
                            let _ = tx_clone.send(chunk);
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(PtyError::Io)?;

        Ok(Self {
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
            output_tx,
            ring_buffer,
            _reader_thread: Some(reader_thread),
        })
    }

    /// Writes raw bytes to the child stdin via PTY master.
    pub fn write(&self, bytes: &[u8]) -> Result<(), PtyError> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Delivers a prompt string and trailing return to the child.
    pub fn deliver_prompt(&self, text: &str, bracketed_paste: bool) -> Result<(), PtyError> {
        let formatted = format_prompt(text, bracketed_paste);
        self.write(&formatted)
    }

    /// Resizes the PTY window dimensions.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let master = self.master.lock().unwrap();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Pty(e.to_string()))?;
        Ok(())
    }

    /// Terminates the child process.
    pub fn kill(&self) -> Result<(), PtyError> {
        let mut child = self.child.lock().unwrap();
        child.kill().map_err(|e| PtyError::Pty(e.to_string()))?;
        Ok(())
    }

    /// Waits for the child to exit and returns its exit status.
    pub fn wait(&self) -> Result<portable_pty::ExitStatus, PtyError> {
        let mut child = self.child.lock().unwrap();
        let status = child.wait().map_err(|e| PtyError::Pty(e.to_string()))?;
        Ok(status)
    }

    /// Checks if the child process is still running.
    pub fn is_alive(&self) -> bool {
        let mut child = self.child.lock().unwrap();
        matches!(child.try_wait(), Ok(None))
    }

    /// Returns a receiver for realtime broadcast of PTY output chunks.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output_tx.subscribe()
    }

    /// Returns a copy of the ring buffer containing recent output (up to 256 KiB).
    pub fn recent_output(&self) -> Vec<u8> {
        self.ring_buffer.lock().unwrap().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_capacity() {
        let mut rb = RingBuffer::new(10);
        rb.push_slice(b"hello");
        assert_eq!(rb.len(), 5);
        assert_eq!(rb.to_vec(), b"hello");

        rb.push_slice(b"world!");
        // "helloworld!" len 11 > 10, overflows by 1 -> "elloworld!"
        assert_eq!(rb.len(), 10);
        assert_eq!(rb.to_vec(), b"elloworld!");

        rb.push_slice(b"0123456789extra");
        // len 15 >= 10 -> takes last 10 bytes: "56789extra"
        assert_eq!(rb.len(), 10);
        assert_eq!(rb.to_vec(), b"56789extra");
    }

    #[test]
    fn test_format_prompt() {
        // Single line without bracketed paste
        assert_eq!(format_prompt("hello", false), b"hello\r");

        // Multi-line without bracketed paste
        assert_eq!(format_prompt("hello\nworld", false), b"hello\nworld\r");

        // Single line with bracketed paste requested (no bracket needed)
        assert_eq!(format_prompt("hello", true), b"hello\r");

        // Multi-line with bracketed paste
        assert_eq!(
            format_prompt("line1\nline2", true),
            b"\x1b[200~line1\nline2\x1b[201~\r"
        );
    }
}
