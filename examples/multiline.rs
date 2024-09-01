//! An example using an unthreaded status line with a more fancy layout.
//!
//! This example is similar to the basic unthreaded example, but uses a more sophisticated status
//! line layout that fills the entire line.

use std::io::{self, Stdout, Write};
use std::thread;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::style::{Color, Print, SetForegroundColor};
use tracing::info;

use tracing_statusbar::Builder;

/// A helper struct that hides the cursor when created, and restores it when dropped.
struct HiddenCursorGuard(Stdout);

impl HiddenCursorGuard {
    fn new(mut stdout: Stdout) -> io::Result<Self> {
        // Hide the cursor
        crossterm::queue!(stdout, Hide)?;

        Ok(Self(stdout))
    }
}

impl Drop for HiddenCursorGuard {
    fn drop(&mut self) {
        // Show the cursor
        crossterm::execute!(self.0, Show).expect("Could not reset cursor");
    }
}

/// A status line printing callback. This should print the status line to the provided writer and
/// return the number of newlines written.
fn write_status_line<W: Write>(output: &mut W) -> io::Result<u16> {
    // Write the status line. Note that for a multi-line status the last line should not have a
    // newline at the end, so that the status lines stay at the bottom of the screen. Also note the
    // use of `queue!` here, which does not flush the output writer. This is done implicitly by the
    // crate.
    crossterm::queue!(
        output,

        SetForegroundColor(Color::Red),
        Print("This is the first line.\n"),

        SetForegroundColor(Color::Green),
        Print("This is the second line.\n"),

        SetForegroundColor(Color::Blue),
        Print("This is the third line."),
    )?;

    // Return the number of newlines written.
    Ok(2)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a guard that keeps the cursor hidden
    let _guard = HiddenCursorGuard::new(io::stdout())?;

    // Create the status line log writer
    let writer = Builder::with_stdout()
        .with_callback(write_status_line)
        .finish();

    // Create a subscriber and attach the writer to it
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .finish();

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)?;

    // Do some work
    for count in 0..10 {
        info!("This is log message {count}");
        thread::sleep(Duration::from_millis(1000));
    }

    info!("All done");
    Ok(())
}
