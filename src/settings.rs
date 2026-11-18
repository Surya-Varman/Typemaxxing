//! User settings (mode and length), remembered between runs in settings.json
//! next to the history file.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::typing::Mode;

pub const TIME_OPTIONS: &[u64] = &[15, 30, 60, 120];
pub const WORD_OPTIONS: &[usize] = &[10, 25, 50, 100];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Time,
    Words,
}

/// Accent colour for the cursor, progress bar, highlights and borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Purple,
    Pink,
    Yellow,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Purple, Theme::Pink, Theme::Yellow];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Purple => "purple",
            Theme::Pink => "pink",
            Theme::Yellow => "yellow",
        }
    }

    /// 256-colour palette entries, which every common terminal (including
    /// macOS Terminal) supports.
    pub fn color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Theme::Purple => Color::Indexed(141), // #af87ff
            Theme::Pink => Color::Indexed(212),   // #ff87d7
            Theme::Yellow => Color::Indexed(221), // #ffd75f
        }
    }

    /// `/color` ← / →: step through the themes, wrapping around.
    pub fn step(self, forward: bool) -> Theme {
        let i = Theme::ALL.iter().position(|&t| t == self).unwrap_or(0);
        let n = Theme::ALL.len();
        Theme::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub kind: Kind,
    /// Seconds for a timed test.
    pub time: u64,
    /// Word count for a words test.
    pub words: usize,
    /// Ask "Quit typemaxxing?" before quitting on Esc.
    pub confirm_quit: bool,
    /// Size of the practice text: 1 = normal, 2+ = big block letters.
    pub font_size: u8,
    /// Accent colour theme.
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            kind: Kind::Time,
            time: 30,
            words: 25,
            confirm_quit: true,
            font_size: 1,
            theme: Theme::Purple,
        }
    }
}

impl Settings {
    pub fn mode(&self) -> Mode {
        match self.kind {
            Kind::Time => Mode::Time(self.time),
            Kind::Words => Mode::Words(self.words),
        }
    }

    /// `/fontSize` ↑ / ↓.
    pub fn change_font_size(&mut self, bigger: bool) {
        use crate::bigtext::{MAX_SIZE, MIN_SIZE};
        self.font_size = if bigger {
            (self.font_size + 1).min(MAX_SIZE)
        } else {
            self.font_size.saturating_sub(1).max(MIN_SIZE)
        };
    }

    /// `m`: switch between a timed test and a word-count test.
    pub fn toggle_kind(&mut self) {
        self.kind = match self.kind {
            Kind::Time => Kind::Words,
            Kind::Words => Kind::Time,
        };
    }

    /// `s` (forward) / `S` (backward): step through the length options for the current mode.
    pub fn cycle_length(&mut self, forward: bool) {
        match self.kind {
            Kind::Time => self.time = cycle(TIME_OPTIONS, self.time, forward),
            Kind::Words => self.words = cycle(WORD_OPTIONS, self.words, forward),
        }
    }

    /// Length options for the current mode, as labels, plus the index of the
    /// selected one (None if a custom value from the command line is in use).
    pub fn length_options(&self) -> (Vec<String>, Option<usize>) {
        match self.kind {
            Kind::Time => (
                TIME_OPTIONS.iter().map(|t| format!("{t}s")).collect(),
                TIME_OPTIONS.iter().position(|&t| t == self.time),
            ),
            Kind::Words => (
                WORD_OPTIONS.iter().map(|w| w.to_string()).collect(),
                WORD_OPTIONS.iter().position(|&w| w == self.words),
            ),
        }
    }
}

/// Next/previous option, wrapping around. A custom value not in the list
/// jumps to the nearest option in the chosen direction.
fn cycle<T: Copy + PartialOrd>(options: &[T], current: T, forward: bool) -> T {
    match options.iter().position(|&o| o == current) {
        Some(i) if forward => options[(i + 1) % options.len()],
        Some(i) => options[(i + options.len() - 1) % options.len()],
        None if forward => *options
            .iter()
            .find(|&&o| o > current)
            .unwrap_or(&options[0]),
        None => *options
            .iter()
            .rev()
            .find(|&&o| o < current)
            .unwrap_or(&options[options.len() - 1]),
    }
}

fn path() -> Result<PathBuf> {
    Ok(crate::paths::data_dir()?.join("settings.json"))
}

/// Loads saved settings, falling back to defaults if missing or unreadable.
pub fn load() -> Settings {
    path()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> Result<()> {
    let path = path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, serde_json::to_string_pretty(settings)?)
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_cycle() {
        let mut s = Settings::default();
        assert_eq!(s.mode(), Mode::Time(30));
        s.cycle_length(true);
        assert_eq!(s.mode(), Mode::Time(60));
        s.cycle_length(true);
        s.cycle_length(true);
        assert_eq!(s.mode(), Mode::Time(15)); // wrapped around
        s.cycle_length(false);
        assert_eq!(s.mode(), Mode::Time(120));
        s.toggle_kind();
        assert_eq!(s.mode(), Mode::Words(25));
        s.toggle_kind();
        assert_eq!(s.mode(), Mode::Time(120)); // remembers time choice
    }

    #[test]
    fn custom_value_cycles_to_nearest() {
        assert_eq!(cycle(TIME_OPTIONS, 45, true), 60);
        assert_eq!(cycle(TIME_OPTIONS, 45, false), 30);
        assert_eq!(cycle(TIME_OPTIONS, 500, true), 15);
    }

    #[test]
    fn old_or_partial_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"kind":"words"}"#).unwrap();
        assert_eq!(s.mode(), Mode::Words(25));
        assert!(
            s.confirm_quit,
            "files saved before this setting existed get the default"
        );
        assert_eq!(s.font_size, 1);
        assert_eq!(s.theme, Theme::Purple, "purple is the default");
    }

    #[test]
    fn font_size_stays_in_range() {
        let mut s = Settings::default();
        s.change_font_size(false);
        assert_eq!(s.font_size, 1);
        for _ in 0..50 {
            s.change_font_size(true);
        }
        assert_eq!(s.font_size, crate::bigtext::MAX_SIZE);
    }

    #[test]
    fn theme_steps_and_wraps() {
        assert_eq!(Theme::Purple.step(true), Theme::Pink);
        assert_eq!(Theme::Yellow.step(true), Theme::Purple);
        assert_eq!(Theme::Purple.step(false), Theme::Yellow);
        let s: Settings = serde_json::from_str(r#"{"theme":"pink"}"#).unwrap();
        assert_eq!(s.theme, Theme::Pink);
    }
}
