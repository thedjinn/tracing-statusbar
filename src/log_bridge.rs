use std::io::{self, Write};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, SendError, SyncSender};

/// A log entry sender. This is used to send log entries to a consumer on a background thread.
/// Propagation of entries is done by means of an mpsc channel. The sender and receiver share a
/// second channel to propagate back consumed buffers. This reduces the number of allocations made
/// by the log sender.
///
/// The `LogSender` implements `Write` (hence the need for a pool of buffers instead of taking
/// ownership of entries) and thus can be passed to `tracing_subscriber` as the return type of a
/// `MakeWriter` impl.
#[derive(Clone)]
pub struct LogSender {
    /// A sender that propagates log message buffers to a LogReceiver instance. Sending an empty
    /// message indicates that the receiver should stop processing. An explicit closing message is
    /// used here so that log senders do not need to have their lifetimes managed and no blocking
    /// synchronization is required.
    sender: SyncSender<Option<Vec<u8>>>,

    /// A free list of log message buffers.
    pool: Arc<Mutex<Receiver<Vec<u8>>>>,
}

impl LogSender {
    /// Close the log sender. No further entries can be sent through the channel after this.
    ///
    /// To prevent any missing log messages the `Write` impl will print messages directly to
    /// stdout if they were written after the channel was closed. Under normal operation this is
    /// not a concern, because the sender should be used until the program is shut down, or the
    /// sender is replaced with another log consumer. The stdout fallback merely exists as a
    /// debugging aid.
    pub fn close(&mut self) {
        let _ = self.sender.send(None);
    }
}

impl Write for LogSender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let pool = self.pool.lock().expect("Pool mutex was poisoned");

        let buffer = match pool.try_recv() {
            Ok(mut buffer) => {
                buffer.truncate(0);
                buffer.extend(buf);
                buffer
            },

            // An empty or closed pool should allocate a new buffer
            Err(_) => buf.to_owned(),
        };

        // Release the lock, critical section ends here
        drop(pool);

        match self.sender.send(Some(buffer)) {
            Ok(()) => (),

            // Directly print logs if the reader is closed
            Err(SendError(buffer)) => print!("{}", std::str::from_utf8(&buffer.unwrap()).unwrap_or("")),
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An enumeration that lists the things that can go wrong when trying to receive data from a
/// LogRecever.
pub enum TryRecvError {
    /// There are no new log entries to be processed.
    Empty,

    /// The channel is closed.
    Closed,
}

impl From<mpsc::TryRecvError> for TryRecvError {
    fn from(value: mpsc::TryRecvError) -> Self {
        match value {
            mpsc::TryRecvError::Empty => Self::Empty,
            mpsc::TryRecvError::Disconnected => Self::Closed,
        }
    }
}

/// A log entry. This contains a buffer and a sender to propagate the buffer back into the buffer
/// pool.
pub struct LogEntry {
    /// The buffer containing the log entry. This is normally always `Some`, until the Drop impl is
    /// called, which takes the entry and sends it back into the buffer pool.
    buffer: Option<Vec<u8>>,

    /// A sender to a pool of unused buffers.
    pool: SyncSender<Vec<u8>>,
}

impl Drop for LogEntry {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            let _ = self.pool.send(buffer);
        }
    }
}

impl Deref for LogEntry {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buffer.as_ref().unwrap()
    }
}

/// A receiver for log entries.
pub struct LogReceiver {
    /// The channel used to propagate buffers.
    receiver: Receiver<Option<Vec<u8>>>,

    /// A sender used to return used buffers to a pool for reuse.
    pool: SyncSender<Vec<u8>>,
}

impl LogReceiver {
    /// Wait for the next log entry to arrive, wrapping it in a `LogEntry` struct. Returns `None`
    /// when the last `LogSender` was dropped.
    pub fn recv(&mut self) -> Option<LogEntry> {
        self.receiver
            .recv()
            .ok()
            .flatten()
            .map(|buffer| LogEntry {
                buffer: Some(buffer),
                pool: self.pool.clone(),
            })
    }

    /// Try to receive a next log entry without blocking, wrapping it in a `LogEntry`. Returns
    /// either the received entry or a `TryRecvError` indicating why a log entry could not be
    /// retrieved.
    pub fn try_recv(&mut self) -> Result<LogEntry, TryRecvError> {
        self.receiver
            .try_recv()
            .map_err(TryRecvError::from)?
            .ok_or(TryRecvError::Closed)
            .map(|buffer| LogEntry {
                buffer: Some(buffer),
                pool: self.pool.clone(),
            })
    }
}

/// Initialize a new log sender/receiver pair.
pub fn init() -> (LogSender, LogReceiver) {
    // TODO: Determine proper default backpressure
    // TODO: Make backpressure optional
    // TODO: Make backpressure customizable
    let (sender, receiver) = mpsc::sync_channel(1024);
    let (pool_sender, pool_receiver) = mpsc::sync_channel(1024);

    (
        LogSender {
            sender,
            pool: Arc::new(Mutex::new(pool_receiver)),
        },

        LogReceiver {
            receiver,
            pool: pool_sender,
        },
    )
}
