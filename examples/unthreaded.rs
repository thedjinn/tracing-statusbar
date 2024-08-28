//! A simple example using an unthreaded status line writer.
//!
//! This example counts to 10, with a sleep between each count. It is a demonstration of the most
//! basic use case of the crate.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crossterm::style::Print;
use tracing::info;

use tracing_statusbar::Builder;

/// A status line printing callback. This should print the status line to the provided writer and
/// return the number of newlines written.
fn write_status_line<W>(mut output: &W) -> io::Result<u16>
where
    for<'a> &'a W: Write,
{
    // Write the status line. Note that for a single line no newlines should be emitted, so that
    // the status line stays at the bottom of the screen. Also note the use of `queue!` here, which
    // does not flush the output writer. This is done implicitly by the crate.
    crossterm::queue!(
        output,
        Print("--- This is the status bar ---"),
    )?;

    // Return the number of newlines written, which is zero for a single status line.
    Ok(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
