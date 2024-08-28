use std::io::{self, Write};
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, SendError, SyncSender};

// TODO: Rename to LogSender
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

pub enum TryRecvError {
    Empty,
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

//pub enum LogEntry {
    //Close,

    //Entry {
        //buffer: Option<Vec<u8>>,
        //pool: SyncSender<Vec<u8>>,
    //}
//}

pub struct LogEntry {
    buffer: Option<Vec<u8>>,
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

// TODO: Rename to LogReceiver
pub struct LogReceiver {
    receiver: Receiver<Option<Vec<u8>>>,
    pool: SyncSender<Vec<u8>>,
}

impl LogReceiver {
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
