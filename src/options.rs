//! How a run behaves and looks while you type — the `[typing]` section of
//! the config, the Settings screen's typing rows and a few command-line
//! switches. Resolved once at start (and on each Settings change) into
//! `App::options`.

/// The resolved `[typing]` settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypingOptions {
    /// Show WPM and accuracy while typing (the clock and mode always show).
    pub live_wpm: bool,
    /// Blind mode: typed text isn't colored right or wrong, and no numbers
    /// show until the run ends.
    pub blind: bool,
}

impl Default for TypingOptions {
    fn default() -> Self {
        Self {
            live_wpm: true,
            blind: false,
        }
    }
}

impl TypingOptions {
    /// Whether the live WPM and accuracy are drawn: wanted, and not blind.
    pub fn show_live_numbers(self) -> bool {
        self.live_wpm && !self.blind
    }
}
