//! A more advanced example using an unthreaded stateful status line writer where state is shared
//! between the status line and the application.
//!
//! In this example the shared state is implemented as a struct that is wrapped in an `Arc<Mutex>`,
//! of which a clone is passed into the status line callback.
//!
//! The downside of this approach is that a wrapping closure is necessary when invoking the status
//! line builder. This closure needs an explicit type annotation, and so is not as flexible as a
//! full `MakeCallback` implementation.
//!
//! Compare also to the `shared_state.rs` example, which provides an alternative pattern that does
//! offer full flexibility and generic typing.

use std::io::{self, Stdout, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::style::Print;
use tracing::info;

use tracing_statusbar::Builder;

/// A struct that represents the status line and its state.
#[derive(Default)]
struct StatusLine {
    /// The number of times the status line callback was invoked.
    count: u32,

    /// A progress indicator for the application's main loop.
    progress: f32,
}

impl StatusLine {
    fn render<T: Write>(&mut self, output: &mut T) -> io::Result<u16> {
        // Increment the counter
        self.count += 1;

        // Write the status line. Note that for a single line no newlines should be emitted, so
        // that the status line stays at the bottom of the screen. Also note the use of
        // `queue!` here, which does not flush the output writer. This is done implicitly by
        // the crate.
        crossterm::queue!(
            output,
            Print(format!("--- The statusbar was redrawn {} times, progress is {:.1}% ---", self.count, self.progress)),
        )?;

        // Return the number of newlines written, which is zero for a single status line.
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the status line state
    let status_line = Arc::new(Mutex::new(StatusLine::default()));

    // Create the status line log writer and provide it with a closure that locks and calls into
    // the status line struct.
    let writer = Builder::with_stdout()
        .with_callback({
            let status_line = status_line.clone();

            move |output: &mut Stdout| {
                status_line
                    .lock()
                    .expect("Status line mutex was poisoned")
                    .render(output)
            }
        })
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

        // Update the progress value
        status_line
            .lock()
            .expect("Status line mutex was poisoned")
            .progress = ((count + 1) as f32 / 10.0) * 100.0;
    }

    info!("All done");
    Ok(())
}
