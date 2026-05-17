use std::time::{Duration, Instant};

use crate::game::{get_char_states, CharState};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Typing,
    Results,
    Stats,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Time(u64),       // seconds
    Words(usize),    // word count
    Quote,
    Code(crate::words::CodeLang),
    Zen,
    Custom,
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Time(s) => format!("time-{}s", s),
            Mode::Words(n) => format!("words-{}", n),
            Mode::Quote => "quote".to_string(),
            Mode::Code(lang) => format!("code-{:?}", lang).to_lowercase(),
            Mode::Zen => "zen".to_string(),
            Mode::Custom => "custom".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Word {
    pub text: String,
    pub typed: String,
    pub submitted: bool,
}

impl Word {
    pub fn new(text: String) -> Self {
        Self { text, typed: String::new(), submitted: false }
    }
}

pub struct MenuItem {
    pub label: &'static str,
    pub mode: Option<Mode>,
    pub action: MenuAction,
}

#[derive(Clone, Copy)]
pub enum MenuAction {
    Start,
    ShowStats,
    Quit,
}

pub fn default_menu() -> Vec<MenuItem> {
    use crate::words::CodeLang;
    vec![
        MenuItem { label: "Time · 15s",  mode: Some(Mode::Time(15)),  action: MenuAction::Start },
        MenuItem { label: "Time · 30s",  mode: Some(Mode::Time(30)),  action: MenuAction::Start },
        MenuItem { label: "Time · 60s",  mode: Some(Mode::Time(60)),  action: MenuAction::Start },
        MenuItem { label: "Time · 120s", mode: Some(Mode::Time(120)), action: MenuAction::Start },
        MenuItem { label: "Words · 10",  mode: Some(Mode::Words(10)), action: MenuAction::Start },
        MenuItem { label: "Words · 25",  mode: Some(Mode::Words(25)), action: MenuAction::Start },
        MenuItem { label: "Words · 50",  mode: Some(Mode::Words(50)), action: MenuAction::Start },
        MenuItem { label: "Words · 100", mode: Some(Mode::Words(100)),action: MenuAction::Start },
        MenuItem { label: "Quote",       mode: Some(Mode::Quote),     action: MenuAction::Start },
        MenuItem { label: "Code · Rust",       mode: Some(Mode::Code(CodeLang::Rust)),       action: MenuAction::Start },
        MenuItem { label: "Code · Python",     mode: Some(Mode::Code(CodeLang::Python)),     action: MenuAction::Start },
        MenuItem { label: "Code · JavaScript", mode: Some(Mode::Code(CodeLang::JavaScript)), action: MenuAction::Start },
        MenuItem { label: "Zen",         mode: Some(Mode::Zen),       action: MenuAction::Start },
        MenuItem { label: "Stats",       mode: None,                  action: MenuAction::ShowStats },
        MenuItem { label: "Quit",        mode: None,                  action: MenuAction::Quit },
    ]
}

pub struct App {
    pub screen: Screen,
    pub previous_screen: Screen,
    pub menu: Vec<MenuItem>,
    pub menu_index: usize,

    pub mode: Mode,
    pub words: Vec<Word>,
    pub current_word: usize,
    pub started_at: Option<Instant>,
    pub ended_at: Option<Instant>,
    pub correct_chars: usize,
    pub total_typed_chars: usize,
    pub backspaces: usize,
    pub custom_file: Option<String>,
    pub should_quit: bool,
    pub error_message: Option<String>,
    pub tick_count: u64,
}

impl App {
    pub fn new(custom_file: Option<String>) -> Self {
        Self {
            screen: Screen::Menu,
            previous_screen: Screen::Menu,
            menu: default_menu(),
            menu_index: 0,
            mode: Mode::Words(25),
            words: vec![],
            current_word: 0,
            started_at: None,
            ended_at: None,
            correct_chars: 0,
            total_typed_chars: 0,
            backspaces: 0,
            custom_file,
            should_quit: false,
            error_message: None,
            tick_count: 0,
        }
    }

    pub fn elapsed(&self) -> Duration {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => end.duration_since(start),
            (Some(start), None) => start.elapsed(),
            _ => Duration::ZERO,
        }
    }

    pub fn elapsed_minutes(&self) -> f64 {
        self.elapsed().as_secs_f64() / 60.0
    }

    pub fn wpm(&self) -> f64 {
        let minutes = self.elapsed_minutes();
        if minutes <= 0.0 { return 0.0; }
        (self.correct_chars as f64 / 5.0) / minutes
    }

    pub fn accuracy(&self) -> f64 {
        if self.total_typed_chars == 0 { return 100.0; }
        (self.correct_chars as f64 / self.total_typed_chars as f64) * 100.0
    }

    pub fn time_remaining(&self) -> Option<Duration> {
        if let Mode::Time(secs) = self.mode {
            let total = Duration::from_secs(secs);
            let elapsed = self.elapsed();
            if elapsed >= total {
                Some(Duration::ZERO)
            } else {
                Some(total - elapsed)
            }
        } else {
            None
        }
    }

    pub fn progress(&self) -> Option<(usize, usize)> {
        if let Mode::Words(n) = self.mode {
            Some((self.current_word.min(n), n))
        } else {
            None
        }
    }

    pub fn start_game(&mut self, mode: Mode) -> anyhow::Result<()> {
        use crate::words;
        self.mode = mode;
        self.words = match mode {
            Mode::Time(_) => words::random_words(300).into_iter().map(Word::new).collect(),
            Mode::Words(n) => words::random_words(n).into_iter().map(Word::new).collect(),
            Mode::Quote => words::random_quote().into_iter().map(Word::new).collect(),
            Mode::Code(lang) => words::random_code_snippet(lang).into_iter().map(Word::new).collect(),
            Mode::Zen => words::random_words(500).into_iter().map(Word::new).collect(),
            Mode::Custom => {
                if let Some(path) = &self.custom_file {
                    let w = words::words_from_file(path)?;
                    if w.is_empty() {
                        return Err(anyhow::anyhow!("custom file is empty"));
                    }
                    w.into_iter().map(Word::new).collect()
                } else {
                    return Err(anyhow::anyhow!("no custom file provided"));
                }
            }
        };
        self.current_word = 0;
        self.started_at = None;
        self.ended_at = None;
        self.correct_chars = 0;
        self.total_typed_chars = 0;
        self.backspaces = 0;
        self.screen = Screen::Typing;
        Ok(())
    }

    pub fn restart(&mut self) -> anyhow::Result<()> {
        let mode = self.mode;
        self.start_game(mode)
    }

    pub fn ensure_timer_started(&mut self) {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
    }

    pub fn finish_game(&mut self) {
        if self.ended_at.is_none() {
            self.ended_at = Some(Instant::now());
        }
        // Tally remaining current word characters for accuracy
        self.tally_current_word();
        self.screen = Screen::Results;
    }

    fn tally_current_word(&mut self) {
        // Already tallied on each keypress via correct_chars/total_typed_chars accounting.
        // No-op kept for clarity.
    }

    pub fn handle_char(&mut self, c: char) {
        self.ensure_timer_started();
        if self.current_word >= self.words.len() {
            return;
        }

        // Space = submit
        if c == ' ' {
            self.submit_word();
            return;
        }

        let word = &mut self.words[self.current_word];
        let target_chars: Vec<char> = word.text.chars().collect();
        let pos = word.typed.chars().count();

        word.typed.push(c);
        self.total_typed_chars += 1;
        if let Some(&tc) = target_chars.get(pos) {
            if tc == c {
                self.correct_chars += 1;
            }
        }
        // Extra characters do not count as correct.
    }

    pub fn handle_backspace(&mut self, ctrl: bool) {
        if self.current_word >= self.words.len() {
            return;
        }
        let word = &mut self.words[self.current_word];
        if word.typed.is_empty() {
            // Move back to previous word if it was submitted (not space-locked)
            if self.current_word > 0 {
                self.current_word -= 1;
                let prev = &mut self.words[self.current_word];
                prev.submitted = false;
                // Subtract the implicit space we counted when submitting
                if self.correct_chars > 0 {
                    // The space char was either correct or not — we only counted correct ones.
                    // Conservative: leave correct_chars alone here; we adjusted on submit.
                }
            }
            return;
        }
        if ctrl {
            // delete entire word from typed
            self.backspaces += word.typed.chars().count();
            word.typed.clear();
        } else {
            word.typed.pop();
            self.backspaces += 1;
        }
    }

    fn submit_word(&mut self) {
        if self.current_word >= self.words.len() {
            return;
        }
        // Count the space as a typed character
        self.total_typed_chars += 1;
        let word = &mut self.words[self.current_word];
        word.submitted = true;
        // A "correctly typed" word adds the trailing space as a correct char.
        let states = get_char_states(&word.text, &word.typed);
        let all_correct = !states.is_empty()
            && states.iter().all(|(_, s)| *s == CharState::Correct)
            && word.typed.chars().count() == word.text.chars().count();
        if all_correct {
            self.correct_chars += 1; // space
        }
        self.current_word += 1;

        // word-mode completion
        if let Mode::Words(n) = self.mode {
            if self.current_word >= n {
                self.finish_game();
                return;
            }
        }
        // quote/code completion
        if matches!(self.mode, Mode::Quote | Mode::Code(_)) && self.current_word >= self.words.len() {
            self.finish_game();
        }
    }

    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.screen != Screen::Typing { return; }
        if let Some(remaining) = self.time_remaining() {
            if self.started_at.is_some() && remaining.is_zero() {
                self.finish_game();
            }
        }
    }
}
