//! A simple example using a threaded status line writer.
//!
//! The main difference with the unthreaded log writer is that in this example the log messages and
//! status lines are written to stdout using a background thread.

use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::cursor::MoveToColumn;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use tracing::info;

use tracing_statusbar::Builder;

/// A status line printing callback. This should print the status line to the provided writer and
/// return the number of newlines written.
fn write_status_line<W>(mut output: &W) -> io::Result<u16>
where
    for<'a> &'a W: Write,
{
    // Show a temporary status indicating that the callback was fired. Normally the status line
    // should not be written twice in this callback, but this is just done to illustrate how the
    // threaded log writer buffers any log messages.
    crossterm::execute!(
        output,
        Print("--- Doing some work ---"),
    )?;

    // Simulate a long operation. Normally the running time of this callback should be kept short,
    // but the sleep shows that the log writer will not block any other threads, and that logs are
    // buffered.
    thread::sleep(Duration::from_millis(500));

    // Write the status line. Note that for a single line no newlines should be emitted, so that
    // the status line stays at the bottom of the screen. Also note the use of `queue!` here, which
    // does not flush the output writer. This is done implicitly by the crate.
    crossterm::queue!(
        output,
        MoveToColumn(0),
        Clear(ClearType::CurrentLine),
        Print("--- Waiting for the next log message ---"),
    )?;

    // Return the number of newlines written, which is zero for a single status line.
    Ok(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the status line log writer
    let writer = Builder::with_stdout()
        .with_callback(write_status_line)
        .threaded()
        .finish();

    // Create a subscriber and attach the writer to it
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .finish();

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)?;

    // Do some work and record the time taken
    let start = Instant::now();

    for count in 0..20 {
        info!("This is log message {count}");
        thread::sleep(Duration::from_millis(300 * (count % 5)));
    }

    // This should show a value very close to 12 seconds, indicating that the log writer thread
    // didn't block the main thread.
    info!("All done, loop took {:?}", start.elapsed());
    Ok(())
}
