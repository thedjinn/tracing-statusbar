use crossterm::terminal;

/// A scope guard for terminal raw mode.
///
/// This struct disables raw mode on initialization and re-enables raw mode when dropped.
pub struct RawModeGuard;

impl RawModeGuard {
    /// Initialize a new raw mode guard.
    ///
    /// This disables terminal raw mode until the returned struct is dropped.
    pub fn new() -> Self {
        terminal::disable_raw_mode()
            .expect("Could not disable terminal raw mode");

        Self
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        terminal::enable_raw_mode()
            .expect("Could not enable terminal raw mode");
    }
}
