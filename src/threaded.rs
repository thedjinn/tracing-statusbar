use std::io::{self, Write};
use std::thread::{self, JoinHandle};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::style::ResetColor;
use crossterm::terminal::{Clear, ClearType};
use tracing_subscriber::fmt::MakeWriter;

use crate::{LogReceiver, LogSender, RawModeGuard};
use crate::log_bridge::{self, TryRecvError};

fn handle_logs<T, W>(
    mut receiver: LogReceiver,
    assume_raw_mode: bool,
    mut callback: T,
    mut output: W,
)
where
    T: FnMut(&W) -> io::Result<u16>,
    for<'b> &'b mut W: Write,
{
    let mut lines = 0;

    while let Some(entry) = receiver.recv() {
        // Move to the beginning of the line and reset the color to default
        crossterm::queue!(
            &mut output,
            MoveToColumn(0),
            ResetColor,
        ).expect("Could not write to output");

        // Erase any lines that were written in the previous callback
        for _ in 0..lines {
            crossterm::queue!(
                &mut output,
                Clear(ClearType::CurrentLine),
                MoveUp(1),
            ).expect("Could not write to output");
        }

        // Erase the current line.
        crossterm::queue!(
            &mut output,
            Clear(ClearType::CurrentLine),
        ).expect("Could not write to output");

        // Disable raw mode if necessary
        let raw_mode_guard = if assume_raw_mode {
            Some(RawModeGuard::new())
        } else {
            None
        };

        // Write the log entry
        let _ = (&mut output).write(&entry).expect("Could not write to output");

        // Grab any additional queued entries to reduce unnecessary status line writing
        loop {
            match receiver.try_recv() {
                Ok(entry) => {
                    let _ = (&mut output).write(&entry).expect("Could not write to output");
                }

                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return,
            }
        }

        // Re-enable raw mode if necessary
        drop(raw_mode_guard);

        // Write the status line and track the number of lines written
        crossterm::execute!(
            &mut output,
            MoveToColumn(0),
        ).expect("Could not write to output");

        lines = callback(&mut output).expect("Could not write to output");

        (&mut output).flush().expect("Could not flush output");
    }
}

pub struct ThreadedHandler {
    log_sender: LogSender,
    join_handle: Option<JoinHandle<()>>,
}

impl ThreadedHandler {
    pub(crate) fn new<T, W>(
        callback: T,
        output: W,
        assume_raw_mode: bool,
    ) -> Self
    where
        T: FnMut(&W) -> io::Result<u16> + Send + 'static,
        W: Send + 'static,
        for<'b> &'b mut W: Write,
    {
        let (log_sender, log_receiver) = log_bridge::init();

        let join_handle = thread::spawn(move || {
            crate::threaded::handle_logs(
                log_receiver,
                assume_raw_mode,
                callback,
                output,
            )
        });

        Self {
            log_sender,
            join_handle: Some(join_handle),
        }
    }
}

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

