use std::io::{self, Stdout, Write};
use std::marker::PhantomData;

use crate::{ThreadedHandler, UnthreadedHandler};

pub trait MakeCallback<W: Write> {
    type Callback: (FnMut(&mut W) -> io::Result<u16>);

    fn make_callback(self) -> Self::Callback;
}

impl<T, W> MakeCallback<W> for T
where
    T: FnMut(&mut W) -> io::Result<u16>,
    W: Write,
{
    type Callback = Self;

    fn make_callback(self) -> Self::Callback {
        self
    }
}

/// Internal namespace to hide trait seal.
mod private {
    /// A trait seal.
    pub trait Sealed {}
}

/// Typestate trait for `Builder`.
pub trait State: private::Sealed {}

/// An uninitialized state for `Builder`. This indicates a builder that does not yet have a status
/// line draw callback assigned to it.
pub struct Uninitialized;

impl State for Uninitialized {}
impl private::Sealed for Uninitialized {}

/// A state for `Builder` that will initialize the log handler with log writing on a background
/// thread.
pub struct Threaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{
    callback: T,
    _marker: PhantomData<W>,
}

impl<T, W> Threaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{
    /// Initialize a new threaded state using the provided callback.
    fn new(callback: T) -> Self {
        Self {
            callback,
            _marker: PhantomData,
        }
    }
}

impl<T, W> State for Threaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{}

impl<T, W> private::Sealed for Threaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{}

/// A state for `Builder` that will initialize the log handler with log writing on the foreground
/// thread.
pub struct Unthreaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{
    callback: T,
    _marker: PhantomData<W>,
}

impl<T, W> Unthreaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{
    /// Initialize a new unthreaded state using the provided callback.
    fn new(callback: T) -> Self {
        Self {
            callback,
            _marker: PhantomData,
        }
    }
}

impl<T, W> State for Unthreaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{}

impl<T, W> private::Sealed for Unthreaded<T, W>
where
    T: MakeCallback<W>,
    W: Write,
{}

/// A builder struct for status line log writers.
pub struct Builder<T, W>
where
    T: State,
    W: Write,
{
    callback: T,
    output: W,
    assume_raw_mode: bool,
}

impl<W: Write> Builder<Uninitialized, W> {
    /// Initialize a new builder for the provided output writer.
    pub fn new(output: W) -> Self {
        Self {
            callback: Uninitialized,
            output,
            assume_raw_mode: false,
        }
    }

}

impl Builder<Uninitialized, Stdout> {
    /// Initialize a new builder using standard output for writing.
    pub fn with_stdout() -> Self {
        Self::new(io::stdout())
    }
}

impl Default for Builder<Uninitialized, Stdout> {
    fn default() -> Self {
        Self::with_stdout()
    }
}

impl<T, W> Builder<T, W>
where
    T: State,
    W: Write,
{
    /// Provide a status line callback to the builder.
    ///
    /// This callback will be invoked every time after writing log messages so that the status line
    /// can be shown again.
    ///
    /// The callback is provided with a Write impl that should be used for writing log messages. It
    /// is not advised to use print/println, but instead write directly into the provided writer.
    ///
    /// The recommended approach is to use crossterm or a similar crate for writing. Placement of
    /// the cursor and cleanup of any previously written status lines is handled automatically and
    /// does not need to be taken care of by the callback.
    ///
    /// The callback should return a result. The success value of the result must represent the
    /// number of newlines written. A single status line should not write any newlines. Therefore
    /// the returned value should always be the number of status lines shown minus one.
    ///
    /// The callback does not have to flush the output writer, this is done automatically.
    pub fn with_callback<C>(self, callback: C) -> Builder<Unthreaded<C, W>, W>
    where
        C: MakeCallback<W>,
    {
        Builder {
            callback: Unthreaded::new(callback),
            output: self.output,
            assume_raw_mode: self.assume_raw_mode,
        }
    }

    /// Signal to the builder that for the lifetime of the log writer the terminal is assumed to be
    /// in raw mode.
    ///
    /// This is useful when combined with a reader that waits for single character presses.
    ///
    /// When enabled, the writer will temporarily disable raw mode when writing log messages, and
    /// enable it again afterwards. This also means that the status line callback will be invoked
    /// with raw mo
    ///
    /// Note: when combined with threaded log handlers and
    /// `tracing::subscriber::set_global_default` the use of `assume_raw_mode` can leave the
    /// program in raw mode even if it disables it. This can happen when the disabling of the raw
    /// mode races with a pending log message, or when writing a log message after manually
    /// disabling raw mode). Therefore the use of raw mode is not recommended with threaded
    /// handlers.
    pub fn assume_raw_mode(mut self) -> Self {
        self.assume_raw_mode = true;
        self
    }
}

impl<T, W> Builder<Unthreaded<T, W>, W>
where
    T: MakeCallback<W> + Send + 'static,
    W: Write + Send + 'static,
{
    /// Tell the builder to create a log handler that writes its log messagse using a background
    /// thread.
    ///
    /// This requires that the provided status line callback and writer implement `Send + 'static`.
    ///
    /// Using a background thread is useful when the number of log messages written is expected to
    /// be large, or when it is not desirable that writing log messages should block for long
    /// periods of time, e.g. in async contexts.
    ///
    /// Note: when combined with `tracing::subscriber::set_global_default` the use of
    /// `assume_raw_mode` can leave the program in raw mode even if it disables it. This can happen
    /// when the disabling of the raw mode races with a pending log message, or when writing a log
    /// message after manually disabling raw mode). Therefore the use of raw mode is not
    /// recommended with threaded handlers.
    pub fn threaded(self) -> Builder<Threaded<T, W>, W> {
        Builder {
            callback: Threaded::new(self.callback.callback),
            output: self.output,
            assume_raw_mode: self.assume_raw_mode,
        }
    }
}

impl<T, W> Builder<Threaded<T, W>, W>
where
    T: MakeCallback<W> + Send + 'static,
    W: Write + Send + 'static,
{
    /// Finish construction of the log handler and return a `MakeWriter` impl.
    ///
    /// This can be passed to `with_writer` on a `tracing_subscriber::fmt::SubscriberBuilder`.
    pub fn finish(self) -> ThreadedHandler {
        ThreadedHandler::new(
            self.callback.callback,
            self.output,
            self.assume_raw_mode,
        )
    }
}

impl<T, W> Builder<Unthreaded<T, W>, W>
where
    T: MakeCallback<W>,
    W: Write,
{
    /// Finish construction of the log handler and return a `MakeWriter` impl.
    ///
    /// This can be passed to `with_writer` on a `tracing_subscriber::fmt::SubscriberBuilder`.
    pub fn finish(self) -> UnthreadedHandler<T::Callback, W> {
        UnthreadedHandler::new(
            self.callback.callback.make_callback(),
            self.output,
            self.assume_raw_mode,
        )
    }
}
