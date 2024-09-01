//! A fully interactive example using terminal raw mode to capture keyboard events.
//!
//! This example enters a keyboard reading loop and emits log messages for every key that is
//! pressed. Pressing escape quits the program. The example uses terminal raw mode to disable the
//! line buffering that standard input normally has, and disabling local echo.
//!
//! The example uses crossterm's synchronous event reading, but async event reading (by using
//! `crossterm::event::EventStream`) is also fully supported. For async applications the use of a
//! threaded log writer is recommended (see the `threaded.rs` example for how to do this).
//!
//! The status bar in this application is static to keep the example simple, but it can easily be
//! replaced by the stateful or state sharing status bars from the other examples.

use std::io::{self, Write};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::style::Print;
use crossterm::terminal;
use tracing::{error, info};

use tracing_statusbar::Builder;

/// A status line printing callback. This should print the status line to the provided writer and
/// return the number of newlines written.
fn write_status_line<W: Write>(output: &mut W) -> io::Result<u16> {
    // Write the status line. Note that for a single line no newlines should be emitted, so that
    // the status line stays at the bottom of the screen. Also note the use of `queue!` here, which
    // does not flush the output writer. This is done implicitly by the crate.
    crossterm::queue!(
        output,
        Print("--- Press a key to trigger log messages, ESC to quit ---"),
    )?;

    // Return the number of newlines written, which is zero for a single status line.
    Ok(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable terminal raw mode to capture individual key presses without having to wait for a line
    // break.
    terminal::enable_raw_mode()?;

    // Create the status line log writer. Note that when using raw mode the builder has to be
    // notified that this is the case to avoid screen corruption. This can happen when log messages
    // contain line breaks themselves.
    let writer = Builder::with_stdout()
        .with_callback(write_status_line)
        .assume_raw_mode()
        .finish();

    // Create a subscriber and attach the writer to it
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .finish();

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Ready to read keyboard events");

    // Read terminal events and log any keypresses
    loop {
        match crossterm::event::read() {
            Ok(Event::Key(event)) => match event {
                KeyEvent {
                    kind: KeyEventKind::Press,
                    code: KeyCode::Esc,
                    ..
                } => {
                    info!("Escape pressed, quitting");
                    break;
                }

                event => {
                    info!("Got a keyboard event: {:?}", event);
                }
            }

            Err(err) => {
                error!("Could not read crossterm event: {}", err);
                break;
            }

            _ => (),
        }
    }

    info!("All done");

    // Disable terminal raw mode
    terminal::disable_raw_mode()?;

    Ok(())
}
