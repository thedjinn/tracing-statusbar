//! An advanced example using an unthreaded stateful status line writer where state is shared
//! between the status line and the application.
//!
//! In this example the shared state is implemented by creating a newtype around an `Arc<Mutex>` of
//! a state struct. The newtype is given a `MakeCallback` impl so that it can be passed to the
//! status line builder.
//!
//! Compare also to the `simple_shared_state.rs` example, which provides an alternative to the
//! `MakeCallback` pattern used here that is more simple but also less flexible.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossterm::style::Print;
use tracing::info;

use tracing_statusbar::{Builder, MakeCallback};

/// A struct that represents the status line state.
#[derive(Default)]
struct StatusLineState {
    /// The number of times the status line callback was invoked.
    count: u32,

    /// A progress indicator for the application's main loop.
    progress: f32,
}

/// A struct representing the status line. It is a wrapper around a shared status line state.
#[derive(Default)]
struct StatusLine(Arc<Mutex<StatusLineState>>);

/// An impl of MakeCallback is added to convert the StatusLine struct into a callback that can be
/// used to render the status line.
///
/// The callback factory takes ownership of self and wraps it into the boxed closure. This allows
/// access to the shared status line state (via internal mutability).
impl<W: Write> MakeCallback<W> for StatusLine {
    type Callback = Box<dyn FnMut(&mut W) -> io::Result<u16> + Send>;

    fn make_callback(self) -> Self::Callback {
        Box::new(move |output| {
            // Lock the mutex to gain access to the status line state
            let mut state = self.0.lock().expect("Status line mutex was poisoned");

            // Increment the counter
            state.count += 1;

            // Write the status line. Note that for a single line no newlines should be emitted, so
            // that the status line stays at the bottom of the screen. Also note the use of
            // `queue!` here, which does not flush the output writer. This is done implicitly by
            // the crate.
            crossterm::queue!(
                output,
                Print(format!("--- The statusbar was redrawn {} times, progress is {:.1}% ---", state.count, state.progress)),
            )?;

            // Return the number of newlines written, which is zero for a single status line.
            Ok(0)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create the status line state
    let state = Arc::new(Mutex::new(StatusLineState::default()));

    // Create the status line log writer and provide it with a status line
    let writer = Builder::with_stdout()
        .with_callback(StatusLine(state.clone()))
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
        state
            .lock()
            .expect("Status line mutex was poisoned")
            .progress = ((count + 1) as f32 / 10.0) * 100.0;
    }

    info!("All done");
    Ok(())
}
