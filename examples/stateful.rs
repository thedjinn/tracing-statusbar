//! An example using an unthreaded stateful status line writer.
//!
//! This example is similar to the basic unthreaded example, but allows the status line to maintain
//! state.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crossterm::style::Print;
use tracing::info;

use tracing_statusbar::Builder;

/// A factory function that creates a status line callback that counts and prints the number of
/// times it was invoked.
fn make_write_status_line<W>() -> impl FnMut(&W) -> io::Result<u16>
where
    for<'a> &'a W: Write,
{
    // Create a counter variable. Ownership is moved into the closure.
    let mut count = 0;

    move |mut output: &W| {
        // Increment the counter
        count += 1;

        // Write the status line. Note that for a single line no newlines should be emitted, so
        // that the status line stays at the bottom of the screen. Also note the use of `queue!`
        // here, which does not flush the output writer. This is done implicitly by the crate.
        crossterm::queue!(
            output,
            Print(format!("--- The statusbar was redrawn {count} times ---")),
        )?;

        // Return the number of newlines written, which is zero for a single status line.
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the status line log writer
    let writer = Builder::with_stdout()
        .with_callback(make_write_status_line())
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
