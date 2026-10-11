//! How a run behaves and looks while you type — the `[typing]` section of
//! the config, the Settings screen's typing rows and a few command-line
//! switches. Resolved once at start (and on each Settings change) into
//! `App::options`.

/// Whether a mistake has to be fixed before you go on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StopOnError {
    /// Mistakes are typed like any key (the default).
    #[default]
    Off,
    /// A wrong key doesn't move the cursor: type the right one.
    Letter,
    /// Wrong letters are typed, but a word with a mistake in it can't be
    /// left — the space (or the end of the text) waits until it's fixed.
    Word,
}

impl StopOnError {
    /// The names `stop_on_error` takes, in the order Settings cycles them.
    pub const NAMES: [&'static str; 3] = ["off", "letter", "word"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// The setting called `name` (case-insensitive), if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "letter" => Some(Self::Letter),
            "word" => Some(Self::Word),
            _ => None,
        }
    }
}

/// How the cursor is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorStyle {
    /// The character under it underlined (the default).
    #[default]
    Underline,
    /// The character under it in a solid block.
    Block,
    /// A thin bar before the character: the terminal's own cursor, so it
    /// takes no room in the text.
    Bar,
}

impl CursorStyle {
    pub const NAMES: [&'static str; 3] = ["underline", "block", "bar"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let at = Self::NAMES
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))?;
        Some([Self::Underline, Self::Block, Self::Bar][at])
    }
}

/// When the terminal's bell sounds as you type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeySound {
    /// Never (the default).
    #[default]
    Off,
    /// On a wrong (or refused) key.
    Errors,
    /// On every key.
    Keys,
}

impl KeySound {
    pub const NAMES: [&'static str; 3] = ["off", "errors", "keys"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let at = Self::NAMES
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))?;
        Some([Self::Off, Self::Errors, Self::Keys][at])
    }
}

/// What the pace cursor moves at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pace {
    /// No pace cursor (the default).
    #[default]
    Off,
    /// Your best for the mode being played (a replay: the original run).
    Best,
    /// A target WPM.
    Wpm(u32),
}

impl Pace {
    /// Fastest target pace.
    pub const MAX_WPM: u32 = 300;
    /// The values Settings cycles through.
    pub const CHOICES: [Pace; 12] = [
        Pace::Off,
        Pace::Best,
        Pace::Wpm(30),
        Pace::Wpm(40),
        Pace::Wpm(50),
        Pace::Wpm(60),
        Pace::Wpm(70),
        Pace::Wpm(80),
        Pace::Wpm(90),
        Pace::Wpm(100),
        Pace::Wpm(120),
        Pace::Wpm(150),
    ];

    /// `off`, `best` or a number of WPM (1 to [`Pace::MAX_WPM`]).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "off" => Some(Pace::Off),
            "best" => Some(Pace::Best),
            number => match number.parse::<u32>() {
                Ok(wpm) if (1..=Self::MAX_WPM).contains(&wpm) => Some(Pace::Wpm(wpm)),
                _ => None,
            },
        }
    }

    /// As written in the config and shown on Settings: `off`, `best`, `60`.
    pub fn name(self) -> String {
        match self {
            Pace::Off => "off".into(),
            Pace::Best => "best".into(),
            Pace::Wpm(wpm) => wpm.to_string(),
        }
    }
}

/// The resolved `[typing]` settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypingOptions {
    /// Show WPM and accuracy while typing (the clock and mode always show).
    pub live_wpm: bool,
    /// Blind mode: typed text isn't colored right or wrong, and no numbers
    /// show until the run ends.
    pub blind: bool,
    /// Whether a mistake has to be fixed before going on.
    pub stop_on_error: StopOnError,
    pub cursor: CursorStyle,
    pub sound: KeySound,
    pub pace: Pace,
}

impl Default for TypingOptions {
    fn default() -> Self {
        Self {
            live_wpm: true,
            blind: false,
            stop_on_error: StopOnError::Off,
            cursor: CursorStyle::Underline,
            sound: KeySound::Off,
            pace: Pace::Off,
        }
    }
}

impl TypingOptions {
    /// Whether the live WPM and accuracy are drawn: wanted, and not blind.
    pub fn show_live_numbers(self) -> bool {
        self.live_wpm && !self.blind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_on_error_names_read_back() {
        for name in StopOnError::NAMES {
            assert_eq!(StopOnError::from_name(name).unwrap().name(), name);
        }
        assert_eq!(StopOnError::from_name("WORD"), Some(StopOnError::Word));
        assert_eq!(StopOnError::from_name("strict"), None);
        for name in CursorStyle::NAMES {
            assert_eq!(CursorStyle::from_name(name).unwrap().name(), name);
        }
        for name in KeySound::NAMES {
            assert_eq!(KeySound::from_name(name).unwrap().name(), name);
        }
        assert_eq!(CursorStyle::from_name("BAR"), Some(CursorStyle::Bar));
        assert_eq!(KeySound::from_name("click"), None);
    }

    #[test]
    fn pace_names_read_back() {
        for pace in Pace::CHOICES {
            assert_eq!(Pace::from_name(&pace.name()), Some(pace));
        }
        assert_eq!(Pace::from_name(" Best "), Some(Pace::Best));
        assert_eq!(Pace::from_name("85"), Some(Pace::Wpm(85)));
        for bad in ["0", "301", "fast", "-5"] {
            assert_eq!(Pace::from_name(bad), None, "{bad}");
        }
    }
}
