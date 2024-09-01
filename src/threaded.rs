use std::io::{self, Write};
use std::thread::{self, JoinHandle};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::style::ResetColor;
use crossterm::terminal::{Clear, ClearType};
use tracing_subscriber::fmt::MakeWriter;

use crate::{LogReceiver, LogSender, MakeCallback, RawModeGuard};
use crate::log_bridge::{self, TryRecvError};

/// The entry point for the background log writing thread.
///
/// This function takes a receiving channel, status line callback, output writer, and settings. It
/// will read log entries from the channel and place a status line below them.
///
/// Incoming log lines are grouped together when they are received faster than they could be
/// written to the writer. This ensures that the status line callback is not invoked unnecessarily,
/// i.e. it is not called when its status line would immediately be overwritten by another log
/// message.
fn handle_logs<T, W>(
    mut receiver: LogReceiver,
    assume_raw_mode: bool,
    mut callback: T,
    mut output: W,
)
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    let mut lines = 0;

    while let Some(entry) = receiver.recv() {
        // Move to the beginning of the line and reset the color to default
        crossterm::queue!(
            output,
            MoveToColumn(0),
            ResetColor,
        ).expect("Could not write to output");

        // Erase any lines that were written in the previous callback
        for _ in 0..lines {
            crossterm::queue!(
                output,
                Clear(ClearType::CurrentLine),
                MoveUp(1),
            ).expect("Could not write to output");
        }

        // Erase the current line.
        crossterm::queue!(
            output,
            Clear(ClearType::CurrentLine),
        ).expect("Could not write to output");

        // Disable raw mode if necessary
        let raw_mode_guard = if assume_raw_mode {
            Some(RawModeGuard::new())
        } else {
            None
        };

        // Write the log entry
        let _ = output.write(&entry).expect("Could not write to output");

        // Grab any additional queued entries to reduce unnecessary status line writing
        loop {
            match receiver.try_recv() {
                Ok(entry) => {
                    let _ = output.write(&entry).expect("Could not write to output");
                }

                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return,
            }
        }

        // Re-enable raw mode if necessary
        drop(raw_mode_guard);

        // Write the status line and track the number of lines written
        crossterm::execute!(
            output,
            MoveToColumn(0),
        ).expect("Could not write to output");

        lines = callback(&mut output).expect("Could not write to output");

        output.flush().expect("Could not flush output");
    }
}

/// A threaded status line log handler.
///
/// The struct implements `MakeWriter`, meaning that instances of this struct can be passed as
/// writers to the `tracing_subscriber` crate so that the status line will always be written below
/// the most recently emitted log message.
///
/// This struct owns a background thread that does the actual writing to the provided `Write` impl.
/// The thread will be joined when the handler is dropped.
///
/// Note that when the hander is used as part of `tracing_subscriber`'s global default subscriber
/// the handler is never dropped, and thus the background thread will also continue run until the
/// program is terminated.
pub struct ThreadedHandler {
    /// A sender used to communicate log messages to the background thread.
    log_sender: LogSender,

    /// A join handle that represents the background thread.
    join_handle: Option<JoinHandle<()>>,
}

impl ThreadedHandler {
    /// Initialize a new handler using the provided status line callback maker, writer, and
    /// settings.
    ///
    /// The provided `MakeCallback` argument must implement `Send + 'static` so that the status
    /// line callback can be created inside the background thread.
    pub(crate) fn new<T, W>(
        callback: T,
        output: W,
        assume_raw_mode: bool,
    ) -> Self
    where
        T: MakeCallback<W> + Send + 'static,
        W: Write + Send + 'static,
    {
        let (log_sender, log_receiver) = log_bridge::init();

        let join_handle = thread::spawn(move || {
            crate::threaded::handle_logs(
                log_receiver,
                assume_raw_mode,
                callback.make_callback(),
                output,
            )
        });

        Self {
            log_sender,
            join_handle: Some(join_handle),
        }
    }
}

/// A `Drop` impl that shuts down and joins the log writing thread.
impl Drop for ThreadedHandler {
    fn drop(&mut self) {
        // Note: drop is not guaranteed to be called if self is used as the global default
        // subscriber.
        self.log_sender.close();

        // Join writer thread
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().expect("The log writing thread paniced");
        }
    }
}

impl<'a> MakeWriter<'a> for ThreadedHandler {
    type Writer = LogSender;

    fn make_writer(&'a self) -> Self::Writer {
        self.log_sender.clone()
    }
}
