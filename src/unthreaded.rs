use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::style::ResetColor;
use crossterm::terminal::{Clear, ClearType};
use tracing_subscriber::fmt::MakeWriter;

use crate::RawModeGuard;

struct WriteState<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    callback: T,
    output: W,
    assume_raw_mode: bool,
    lines: u16,
}

impl<T, W> WriteState<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    fn new(callback: T, output: W, assume_raw_mode: bool) -> Self {
        Self {
            callback,
            output,
            assume_raw_mode,
            lines: 0,
        }
    }

    fn invoke_callback(&mut self) -> io::Result<u16> {
        (self.callback)(&mut self.output)
    }
}

pub struct LogWriter<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
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

pub struct UnthreadedHandler<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    writer: LogWriter<T, W>,
}

impl<T, W> UnthreadedHandler<T, W>
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
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
