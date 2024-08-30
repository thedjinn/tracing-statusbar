//! An example using an unthreaded stateful status line writer.
//!
//! This example is similar to the basic unthreaded example, but allows the status line to maintain
//! state.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crossterm::style::Print;
use tracing::info;

use tracing_statusbar::{Builder, MakeCallback};

/// A struct that represents the status line state. It contains a counter that indicates the number
/// of times the status line callback was invoked.
#[derive(Default)]
struct StatusLine {
    count: u32,
}

/// An impl of MakeCallback is added to convert the StatusLine struct into a callback that can be
/// used to render the status line.
///
/// The callback factory takes ownership of self and wraps it into the boxed closure. This allows
/// access to the contents of the status line struct.
///
/// For shared access to the status line the status line struct or its internals can be wrapped
/// with internal mutability containers.
impl<W: Write> MakeCallback<W> for StatusLine {
    type Callback = Box<dyn FnMut(&mut W) -> io::Result<u16> + Send>;

    fn make_callback(mut self) -> Self::Callback {
        Box::new(move |output| {
            // Increment the counter
            self.count += 1;

            // Write the status line. Note that for a single line no newlines should be emitted, so
            // that the status line stays at the bottom of the screen. Also note the use of
            // `queue!` here, which does not flush the output writer. This is done implicitly by
            // the crate.
            crossterm::queue!(
                output,
                Print(format!("--- The statusbar was redrawn {} times ---", self.count)),
            )?;

            // Return the number of newlines written, which is zero for a single status line.
            Ok(0)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the status line log writer
    let writer = Builder::with_stdout()
        .with_callback(StatusLine::default())
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
