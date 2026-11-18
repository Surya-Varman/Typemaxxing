//! Event loop: reads keys, updates the test, redraws, and saves results.
//!
//! Commands start with `/` (which never appears in the practice text):
//!   /m  switch mode (time / words)
//!   /s  next length (/S for previous)
//!   /h  help
//!   /onePressQuit  quit with a single Esc instead of confirming (on/off)
//!   /fontSize      bigger or smaller practice text (↑ / ↓)
//!   /color         colour theme: purple, pink or yellow (← / →)

use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::history::{self, Record};
use crate::settings::{self, Settings};
use crate::stats::Stats;
use crate::typing::TypingTest;
use crate::ui;

/// Commands longer than one letter. They run as soon as the full name is
/// typed (case doesn't matter).
pub const LONG_COMMANDS: &[&str] = &["onePressQuit", "fontSize", "color"];

/// What the text typed after `/` matches so far.
#[derive(Debug, PartialEq, Eq)]
pub enum CommandMatch {
    /// Typed text is the start of this command.
    Partial(&'static str),
    /// Typed text is exactly this command.
    Exact(&'static str),
    /// Doesn't match any command.
    Unknown,
}

pub fn match_command(typed: &str) -> CommandMatch {
    let typed = typed.to_lowercase();
    for &cmd in LONG_COMMANDS {
        let lower = cmd.to_lowercase();
        if lower == typed {
            return CommandMatch::Exact(cmd);
        }
        if lower.starts_with(&typed) {
            return CommandMatch::Partial(cmd);
        }
    }
    CommandMatch::Unknown
}

/// How long to wait for a key before ticking and redrawing anyway.
const TICK: Duration = Duration::from_millis(10);

pub struct App {
    pub settings: Settings,
    pub test: TypingTest,
    /// All saved results, loaded at startup and appended to as tests finish.
    records: Vec<Record>,
    /// Best WPM for the current mode before this test (for "new best!").
    pub previous_best: Option<f64>,
    /// Set once the finished test has been saved (or failed to save).
    pub save_status: Option<Result<(), String>>,
    /// Results from every test finished in this run, for the exit summary.
    pub completed: Vec<Stats>,
    /// `/` was pressed: holds what's been typed after it (empty right after `/`).
    pub command: Option<String>,
    pub show_help: bool,
    /// The "Quit typemaxxing?" box is showing.
    pub confirm_quit_open: bool,
    /// The quit box paused a running test, so cancelling should resume it.
    resume_after_quit_box: bool,
    /// The `/fontSize` box is showing; ↑ / ↓ change the size.
    pub font_dialog_open: bool,
    /// The `/color` box is showing; ← / → pick a theme.
    pub color_dialog_open: bool,
    quit: bool,
}

impl App {
    pub fn new(settings: Settings) -> Self {
        let records = history::load().unwrap_or_default();
        let mut app = Self {
            settings,
            test: TypingTest::new(settings.mode()),
            records,
            previous_best: None,
            save_status: None,
            completed: Vec::new(),
            command: None,
            show_help: false,
            confirm_quit_open: false,
            resume_after_quit_box: false,
            font_dialog_open: false,
            color_dialog_open: false,
            quit: false,
        };
        app.new_test();
        app
    }

    /// Start a fresh test with the current settings.
    fn new_test(&mut self) {
        let mode = self.settings.mode();
        self.test = TypingTest::new(mode);
        self.previous_best = history::best_wpm(&self.records, mode);
        self.save_status = None;
    }

    fn save_settings(&self) {
        // Tests must not overwrite the real settings file.
        if !cfg!(test) {
            let _ = settings::save(&self.settings); // not worth interrupting the user over
        }
    }

    /// Mode or length changed: remember it and start over with the new mode.
    fn apply_settings(&mut self) {
        self.save_settings();
        self.new_test();
    }

    /// Esc: ask first if the confirmation is on, otherwise quit right away.
    fn request_quit(&mut self) {
        if self.settings.confirm_quit {
            // Freeze the clock while deciding; resume on cancel if we paused it.
            self.resume_after_quit_box = self.test.is_running() && !self.test.is_paused();
            self.test.pause();
            self.confirm_quit_open = true;
        } else {
            self.quit = true;
        }
    }

    fn run_letter_command(&mut self, c: char) {
        match c {
            'm' => {
                self.settings.toggle_kind();
                self.apply_settings();
            }
            's' => {
                self.settings.cycle_length(true);
                self.apply_settings();
            }
            'S' => {
                self.settings.cycle_length(false);
                self.apply_settings();
            }
            'h' if self.show_help => self.show_help = false,
            'h' => {
                // Freeze the clock; the test stays paused after help closes.
                self.test.pause();
                self.show_help = true;
            }
            _ => {}
        }
    }

    fn run_long_command(&mut self, cmd: &str) {
        match cmd {
            "onePressQuit" => {
                self.settings.confirm_quit = !self.settings.confirm_quit;
                self.save_settings();
            }
            "fontSize" => {
                // Resizing reflows the text, so a running test starts over.
                if self.test.has_started() && !self.test.is_finished() {
                    self.new_test();
                }
                self.show_help = false;
                self.font_dialog_open = true;
            }
            "color" => {
                // Doesn't change the layout, so pause rather than restart.
                self.test.pause();
                self.show_help = false;
                self.color_dialog_open = true;
            }
            _ => {}
        }
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        // "Quit typemaxxing?" is showing: Esc again quits, any other key cancels.
        if self.confirm_quit_open {
            self.confirm_quit_open = false;
            if key.code == KeyCode::Esc {
                self.quit = true;
            } else if self.resume_after_quit_box {
                self.test.resume();
            }
            return;
        }

        // `/fontSize` box: ↑ / ↓ (or + / −) resize, Enter or Esc closes.
        if self.font_dialog_open {
            match key.code {
                KeyCode::Up | KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.settings.change_font_size(true);
                    self.save_settings();
                }
                KeyCode::Down | KeyCode::Char('-') => {
                    self.settings.change_font_size(false);
                    self.save_settings();
                }
                KeyCode::Enter | KeyCode::Esc => self.font_dialog_open = false,
                _ => {}
            }
            return;
        }

        // `/color` box: ← / → (or ↑ / ↓) pick a theme, Enter or Esc closes.
        if self.color_dialog_open {
            match key.code {
                KeyCode::Right | KeyCode::Down => {
                    self.settings.theme = self.settings.theme.step(true);
                    self.save_settings();
                }
                KeyCode::Left | KeyCode::Up => {
                    self.settings.theme = self.settings.theme.step(false);
                    self.save_settings();
                }
                KeyCode::Enter | KeyCode::Esc => self.color_dialog_open = false,
                _ => {}
            }
            return;
        }

        // Typing a command after `/`. Checked before help so commands also
        // work while help is open.
        if let Some(mut typed) = self.command.take() {
            match key.code {
                KeyCode::Esc => {} // cancel
                KeyCode::Backspace => {
                    // Deleting past the `/` cancels.
                    if typed.pop().is_some() {
                        self.command = Some(typed);
                    }
                }
                KeyCode::Enter => {
                    if let CommandMatch::Exact(cmd) = match_command(&typed) {
                        self.run_long_command(cmd);
                    }
                }
                // Single-letter commands run straight away.
                KeyCode::Char(c) if typed.is_empty() && matches!(c, 'm' | 's' | 'S' | 'h') => {
                    self.run_letter_command(c);
                }
                KeyCode::Char(c) if !ctrl && !alt => {
                    typed.push(c);
                    match match_command(&typed) {
                        CommandMatch::Exact(cmd) => self.run_long_command(cmd),
                        // Keep it (an unknown one is shown in red until fixed or cancelled).
                        _ => self.command = Some(typed),
                    }
                }
                _ => self.command = Some(typed),
            }
            return;
        }

        // Help is open: Esc, Enter or h closes it; `/` starts a command.
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('h') => self.show_help = false,
                KeyCode::Char('/') => self.command = Some(String::new()),
                _ => {}
            }
            return;
        }

        // Paused mid-test: Tab again for new words, any other key resumes
        // (without being typed).
        if self.test.is_paused() {
            match key.code {
                KeyCode::Tab => self.new_test(),
                KeyCode::Esc => self.request_quit(),
                KeyCode::Char('/') if !ctrl && !alt => self.command = Some(String::new()),
                _ => self.test.resume(),
            }
            return;
        }

        match key.code {
            KeyCode::Esc => self.request_quit(),
            // Mid-test, the first Tab pauses; otherwise Tab gives new words.
            KeyCode::Tab if self.test.is_running() => self.test.pause(),
            KeyCode::Tab => self.new_test(),
            KeyCode::Char('/') if !ctrl && !alt => self.command = Some(String::new()),
            KeyCode::Enter if self.test.is_finished() => self.new_test(),
            _ if self.test.is_finished() => {}
            KeyCode::Backspace if ctrl || alt => self.test.backspace_word(),
            KeyCode::Char('w') if ctrl => self.test.backspace_word(),
            KeyCode::Backspace => self.test.backspace(),
            KeyCode::Char(c) if !ctrl && !alt => self.test.type_char(c),
            _ => {}
        }
    }

    /// Ends timed tests and saves a result exactly once when a test finishes.
    pub fn tick(&mut self) {
        self.test.tick();
        if self.test.is_finished() && self.save_status.is_none() {
            let stats = self.test.stats();
            self.completed.push(stats);
            let record = Record::new(self.test.mode, &stats);
            self.save_status = Some(history::append(&record).map_err(|e| format!("{e:#}")));
            self.records.push(record);
        }
    }

    /// True only when this result beats an earlier one in the same mode.
    /// The first result in a mode has nothing to beat, so it isn't a "best".
    pub fn is_new_best(&self) -> bool {
        let wpm = self.test.stats().wpm;
        self.previous_best.is_some_and(|b| wpm > b)
    }
}

pub fn run(terminal: &mut DefaultTerminal, settings: Settings) -> Result<Vec<Stats>> {
    let mut app = App::new(settings);
    while !app.should_quit() {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        // Poll with a short timeout so the timer and live WPM keep updating
        // and a timed test ends within ~10 ms of its limit.
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
            // Windows reports key releases too; only react to presses.
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
        app.tick();
    }
    Ok(app.completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::Mode;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn app() -> App {
        let mut a = App::new(Settings::default());
        a.records.clear();
        a
    }

    #[test]
    fn slash_commands_change_settings_without_typing() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        assert_eq!(a.command.as_deref(), Some(""));
        press(&mut a, KeyCode::Char('s'));
        assert_eq!(a.test.mode, Mode::Time(60));
        assert!(a.test.typed.is_empty());

        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('m'));
        assert_eq!(a.test.mode, Mode::Words(25));
        assert!(!a.test.has_started());
    }

    #[test]
    fn plain_letters_are_typed_not_commands() {
        let mut a = app();
        press(&mut a, KeyCode::Char('s'));
        press(&mut a, KeyCode::Char('m'));
        assert_eq!(a.test.typed, vec!['s', 'm']);
        assert_eq!(a.test.mode, Mode::Time(30));
    }

    #[test]
    fn unknown_command_cancels() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('x'));
        assert!(a.command.is_some(), "unknown command stays visible");
        press(&mut a, KeyCode::Esc);
        assert!(a.command.is_none() && !a.should_quit(), "esc cancels it");
        assert!(a.test.typed.is_empty());
    }

    #[test]
    fn esc_asks_before_quitting() {
        let mut a = app();
        assert!(a.settings.confirm_quit, "on by default");
        press(&mut a, KeyCode::Esc);
        assert!(a.confirm_quit_open && !a.should_quit());
        press(&mut a, KeyCode::Esc);
        assert!(a.should_quit());
    }

    #[test]
    fn other_key_cancels_quit_without_typing() {
        let mut a = app();
        press(&mut a, KeyCode::Esc);
        press(&mut a, KeyCode::Char('n'));
        assert!(!a.confirm_quit_open && !a.should_quit());
        assert!(a.test.typed.is_empty());
    }

    fn type_command(a: &mut App, cmd: &str) {
        press(a, KeyCode::Char('/'));
        for c in cmd.chars() {
            press(a, KeyCode::Char(c));
        }
    }

    #[test]
    fn one_press_quit_turns_confirmation_off() {
        let mut a = app();
        type_command(&mut a, "onePress");
        assert!(
            a.settings.confirm_quit,
            "nothing happens until the full name"
        );
        assert_eq!(a.command.as_deref(), Some("onePress"));
        for c in "Quit".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        assert!(a.command.is_none(), "runs as soon as the name is complete");
        assert!(!a.settings.confirm_quit);
        press(&mut a, KeyCode::Esc);
        assert!(a.should_quit(), "one Esc quits when confirmation is off");
    }

    #[test]
    fn commands_work_inside_help() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('h'));
        type_command(&mut a, "onepressquit"); // case doesn't matter
        assert!(a.show_help && !a.settings.confirm_quit);
        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('h'));
        assert!(!a.show_help);
    }

    #[test]
    fn backspace_edits_and_then_cancels_command() {
        let mut a = app();
        type_command(&mut a, "ox");
        assert_eq!(match_command("ox"), CommandMatch::Unknown);
        press(&mut a, KeyCode::Backspace);
        assert_eq!(a.command.as_deref(), Some("o"));
        press(&mut a, KeyCode::Backspace);
        press(&mut a, KeyCode::Backspace);
        assert!(a.command.is_none());
        assert!(a.test.typed.is_empty());
    }

    #[test]
    fn font_size_dialog_up_down() {
        let mut a = app();
        type_command(&mut a, "fontSize");
        assert!(a.font_dialog_open);
        press(&mut a, KeyCode::Up);
        press(&mut a, KeyCode::Up);
        assert_eq!(a.settings.font_size, 3);
        press(&mut a, KeyCode::Down);
        assert_eq!(a.settings.font_size, 2);
        press(&mut a, KeyCode::Char('x')); // ignored, not typed
        assert!(a.test.typed.is_empty());
        press(&mut a, KeyCode::Esc);
        assert!(!a.font_dialog_open && !a.should_quit() && !a.confirm_quit_open);
    }

    #[test]
    fn color_dialog_cycles_themes() {
        use crate::settings::Theme;
        let mut a = app();
        assert_eq!(a.settings.theme, Theme::Purple);
        type_command(&mut a, "color");
        assert!(a.color_dialog_open);
        press(&mut a, KeyCode::Right);
        assert_eq!(a.settings.theme, Theme::Pink);
        press(&mut a, KeyCode::Right);
        assert_eq!(a.settings.theme, Theme::Yellow);
        press(&mut a, KeyCode::Left);
        assert_eq!(a.settings.theme, Theme::Pink);
        press(&mut a, KeyCode::Enter);
        assert!(!a.color_dialog_open && a.test.typed.is_empty());
    }

    #[test]
    fn tab_pauses_then_second_tab_gives_new_words() {
        let mut a = app();
        let first_text = a.test.target.clone();
        press(&mut a, KeyCode::Char(first_text[0]));
        press(&mut a, KeyCode::Tab);
        assert!(a.test.is_paused());
        assert_eq!(a.test.typed.len(), 1, "progress kept while paused");
        press(&mut a, KeyCode::Tab);
        assert!(!a.test.has_started(), "second tab starts a fresh test");
        assert!(a.test.typed.is_empty());
    }

    #[test]
    fn any_other_key_resumes_without_typing() {
        let mut a = app();
        let c = a.test.target[0];
        press(&mut a, KeyCode::Char(c));
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Char('z'));
        assert!(!a.test.is_paused());
        assert_eq!(a.test.typed.len(), 1, "the resume key isn't typed");
    }

    #[test]
    fn tab_before_start_gives_new_words_immediately() {
        let mut a = app();
        press(&mut a, KeyCode::Tab);
        assert!(!a.test.is_paused() && !a.test.has_started());
    }

    #[test]
    fn quit_box_pauses_and_cancel_resumes() {
        let mut a = app();
        let c = a.test.target[0];
        press(&mut a, KeyCode::Char(c));
        press(&mut a, KeyCode::Esc);
        assert!(a.confirm_quit_open && a.test.is_paused());
        press(&mut a, KeyCode::Char('n'));
        assert!(!a.test.is_paused(), "cancel resumes");
        // If already paused, cancelling the quit box leaves it paused.
        press(&mut a, KeyCode::Tab);
        press(&mut a, KeyCode::Esc);
        press(&mut a, KeyCode::Char('n'));
        assert!(a.test.is_paused());
    }

    #[test]
    fn help_mid_test_pauses_instead_of_restarting() {
        let mut a = app();
        let c = a.test.target[0];
        press(&mut a, KeyCode::Char(c));
        type_command(&mut a, "h");
        assert!(a.show_help && a.test.is_paused());
        assert_eq!(a.test.typed.len(), 1);
    }

    #[test]
    fn first_result_in_a_mode_is_not_a_personal_best() {
        let mut a = app();
        a.previous_best = None;
        assert!(!a.is_new_best());
        a.previous_best = Some(-1.0); // any real result beats this
        assert!(a.is_new_best());
        a.previous_best = Some(1000.0);
        assert!(!a.is_new_best());
    }

    #[test]
    fn help_opens_and_closes() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('h'));
        assert!(a.show_help);
        press(&mut a, KeyCode::Char('a')); // ignored while help is open
        assert!(a.show_help && a.test.typed.is_empty());
        press(&mut a, KeyCode::Esc);
        assert!(!a.show_help && !a.should_quit());
    }
}
