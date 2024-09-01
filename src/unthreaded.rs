use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::style::ResetColor;
use crossterm::terminal::{Clear, ClearType};
use tracing_subscriber::fmt::MakeWriter;

use crate::RawModeGuard;

/// The internal state for a `LogWriter` instance.
struct WriteState<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// The status line callback that will be invoked after every log message.
    callback: T,

    /// The output writer used to write log messages and status lines to.
    output: W,

    /// When true the wrapped writer is assumed to be a terminal that is using raw mode. This will
    /// ensure that the raw mode is temporarily disabled when writing log messages. This prevents
    /// screen corruption.
    assume_raw_mode: bool,

    /// The number of status lines written in the previous invocation of the status line callback.
    /// This is used to properly clean up the previous status lines when a new log message should
    /// be written.
    lines: u16,
}

impl<T, W> WriteState<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// Initialize a new write state using the provided status line callback, output writer, and
    /// settings.
    fn new(callback: T, output: W, assume_raw_mode: bool) -> Self {
        Self {
            callback,
            output,
            assume_raw_mode,
            lines: 0,
        }
    }

    /// Invoke the status line callback.
    ///
    /// A wrapper function is used to assist the compiler with type inference.
    fn invoke_callback(&mut self) -> io::Result<u16> {
        (self.callback)(&mut self.output)
    }
}

/// A writer that will forward any data written to it, and follow this up with an invocation to a
/// status line callback.
///
/// The writer has internal state that is wrapped in an `Arc` and thus can be cloned freely.
pub struct LogWriter<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// The internal state of the log writer.
    state: Arc<Mutex<WriteState<T, W>>>,
}

impl<T, W> Clone for LogWriter<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<T, W> LogWriter<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// Initialize a new log writer using the provided status line callback, output writer, and
    /// settings.
    fn new(callback: T, output: W, assume_raw_mode: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(WriteState::new(
                callback,
                output,
                assume_raw_mode,
            ))),
        }
    }
}

impl<T, W> Write for LogWriter<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// Take the provided buffer and write it to the wrapped writer, invoking the status line
    /// callback after the write finishes.
    ///
    /// When used in conjunction with `tracing_subscriber` the `write` method is only called with
    /// full log lines. This contract must be upheld if the writer is used in another context, so
    /// that it can ensure that the written output is formatted properly.
    ///
    /// The wrapped output writer is flushed after writing a status line, ensuring that status
    /// lines that don't end with newlines are still visible in terminal environments that use
    /// cooked mode.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().expect("Log writer state mutex was poisoned");

        // Move to the beginning of the line and reset the color to default
        crossterm::queue!(
            state.output,
            MoveToColumn(0),
            ResetColor,
        )?;

        // Erase any lines that were written in the previous callback
        for _ in 0..state.lines {
            crossterm::queue!(
                state.output,
                Clear(ClearType::CurrentLine),
                MoveUp(1),
            )?;
        }

        // Erase the current line.
        crossterm::queue!(
            state.output,
            Clear(ClearType::CurrentLine),
        )?;

        // Disable raw mode if necessary
        let raw_mode_guard = if state.assume_raw_mode {
            Some(RawModeGuard::new())
        } else {
            None
        };

        // Write the log entry
        let bytes_written = state.output.write(buf)?;

        // Re-enable raw mode if necessary
        drop(raw_mode_guard);

        // Write the status line and track the number of lines written
        crossterm::execute!(
            state.output,
            MoveToColumn(0),
        )?;

        state.lines = state.invoke_callback()?;

        state.output.flush()?;

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().expect("Log writer state mutex was poisoned");

        state.output.flush()
    }
}

/// An unthreaded status line log handler.
///
/// The struct implements `MakeWriter`, meaning that instances of this struct can be passed as
/// writers to the `tracing_subscriber` crate so that the status line will always be written below
/// the most recently emitted log message.
pub struct UnthreadedHandler<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// The actual writer used for writing log messages. This is cloned on every `make_writer`
    /// invocation.
    writer: LogWriter<T, W>,
}

impl<T, W> UnthreadedHandler<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    /// Initialize a new handler using the provided status line callback, writer, and settings.
    pub(crate) fn new(callback: T, output: W, assume_raw_mode: bool) -> Self {
        Self {
            writer: LogWriter::new(callback, output, assume_raw_mode),
        }
    }
}

impl<'a, T, W> MakeWriter<'a> for UnthreadedHandler<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    type Writer = LogWriter<T, W>;

    fn make_writer(&'a self) -> Self::Writer {
        self.writer.clone()
    }
}
