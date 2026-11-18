//! State for one typing test: the target text, what's been typed and the timer.

use std::time::{Duration, Instant};

use crate::stats::{self, Stats};
use crate::words;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Type for this many seconds.
    Time(u64),
    /// Type this many words.
    Words(usize),
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Time(s) => format!("time {s}"),
            Mode::Words(n) => format!("words {n}"),
        }
    }
}

/// In timed mode, start with this many words and add more as the user nears the end.
const TIMED_INITIAL_WORDS: usize = 100;
const TIMED_REFILL_MARGIN: usize = 60;

pub struct TypingTest {
    pub mode: Mode,
    pub target: Vec<char>,
    pub typed: Vec<char>,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
    /// Set while the test is paused (Tab mid-test, help, quit box).
    paused_at: Option<Instant>,
    /// Total time spent paused so far; left out of the elapsed time.
    paused_total: Duration,
    /// Every character key pressed (backspace not included).
    keystrokes: usize,
    /// Keystrokes that didn't match the expected character, even if fixed later.
    errors: usize,
}

impl TypingTest {
    pub fn new(mode: Mode) -> Self {
        let count = match mode {
            Mode::Time(_) => TIMED_INITIAL_WORDS,
            Mode::Words(n) => n,
        };
        Self::with_text(mode, &words::generate(count).join(" "))
    }

    pub fn with_text(mode: Mode, text: &str) -> Self {
        Self {
            mode,
            target: text.chars().collect(),
            typed: Vec::new(),
            started_at: None,
            finished_at: None,
            paused_at: None,
            paused_total: Duration::ZERO,
            keystrokes: 0,
            errors: 0,
        }
    }

    pub fn has_started(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.finished_at.is_some()
    }

    /// Typing time so far: from the first key to now (or to the end, or to
    /// the moment it was paused), minus any time spent paused.
    pub fn elapsed(&self) -> Duration {
        let Some(start) = self.started_at else {
            return Duration::ZERO;
        };
        let until = self
            .finished_at
            .or(self.paused_at)
            .unwrap_or_else(Instant::now);
        (until - start).saturating_sub(self.paused_total)
    }

    /// True between the first key and the end of the test.
    pub fn is_running(&self) -> bool {
        self.has_started() && !self.is_finished()
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Freeze the clock. Does nothing unless the test is running.
    pub fn pause(&mut self) {
        if self.is_running() && self.paused_at.is_none() {
            self.paused_at = Some(Instant::now());
        }
    }

    /// Restart the clock, leaving the paused time out of the results.
    pub fn resume(&mut self) {
        if let Some(p) = self.paused_at.take() {
            self.paused_total += p.elapsed();
        }
    }

    /// Seconds left in a timed test (None in word mode).
    pub fn remaining(&self) -> Option<Duration> {
        match self.mode {
            Mode::Time(secs) => Some(Duration::from_secs(secs).saturating_sub(self.elapsed())),
            Mode::Words(_) => None,
        }
    }

    /// Words completed so far: word boundaries (spaces) the cursor has passed.
    pub fn words_done(&self) -> usize {
        let n = self.typed.len().min(self.target.len());
        self.target[..n].iter().filter(|c| **c == ' ').count()
    }

    /// How much of the test is left, from 1.0 (not started) down to 0.0.
    /// Time mode: share of the time limit left. Words mode: share of words left.
    pub fn fraction_left(&self) -> f64 {
        let left = match self.mode {
            Mode::Time(secs) => {
                let rem = self.remaining().unwrap_or_default().as_secs_f64();
                rem / secs as f64
            }
            Mode::Words(n) => {
                if self.is_finished() {
                    0.0
                } else {
                    1.0 - self.words_done() as f64 / n as f64
                }
            }
        };
        left.clamp(0.0, 1.0)
    }

    pub fn type_char(&mut self, c: char) {
        if self.is_finished() || self.is_paused() || self.typed.len() >= self.target.len() {
            return;
        }
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        let expected = self.target[self.typed.len()];
        self.keystrokes += 1;
        if c != expected {
            self.errors += 1;
        }
        self.typed.push(c);

        match self.mode {
            Mode::Words(_) if self.typed.len() == self.target.len() => self.finish(),
            Mode::Time(_) if self.target.len() - self.typed.len() < TIMED_REFILL_MARGIN => {
                let more = words::generate(TIMED_INITIAL_WORDS / 2).join(" ");
                self.target.push(' ');
                self.target.extend(more.chars());
            }
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        if !self.is_finished() {
            self.typed.pop();
        }
    }

    /// Delete back to the start of the current word (Ctrl/Option+Backspace, Ctrl+W).
    pub fn backspace_word(&mut self) {
        if self.is_finished() {
            return;
        }
        while self.typed.last() == Some(&' ') {
            self.typed.pop();
        }
        while matches!(self.typed.last(), Some(c) if *c != ' ') {
            self.typed.pop();
        }
    }

    /// Call regularly; ends a timed test when its time is up.
    pub fn tick(&mut self) {
        let limit = match self.mode {
            Mode::Time(secs) => Duration::from_secs(secs),
            Mode::Words(_) => return,
        };
        if let Some(start) = self.started_at
            && self.finished_at.is_none()
            && self.paused_at.is_none()
            && self.elapsed() >= limit
        {
            // End at exactly the limit (plus any paused time), not "now",
            // so a late tick never stretches the test.
            self.finished_at = Some(start + self.paused_total + limit);
        }
    }

    fn finish(&mut self) {
        if self.finished_at.is_none() {
            self.finished_at = Some(Instant::now());
        }
    }

    pub fn stats(&self) -> Stats {
        stats::compute(
            &self.target,
            &self.typed,
            self.keystrokes,
            self.errors,
            self.elapsed(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(t: &mut TypingTest, s: &str) {
        for c in s.chars() {
            t.type_char(c);
        }
    }

    #[test]
    fn word_mode_finishes_at_end_of_text() {
        let mut t = TypingTest::with_text(Mode::Words(2), "hi there");
        assert!(!t.has_started());
        type_str(&mut t, "hi ther");
        assert!(t.has_started() && !t.is_finished());
        t.type_char('e');
        assert!(t.is_finished());
        t.type_char('x'); // ignored after finishing
        assert_eq!(t.typed.len(), 8);
    }

    #[test]
    fn fixed_mistakes_still_count_against_accuracy() {
        let mut t = TypingTest::with_text(Mode::Words(1), "cat");
        type_str(&mut t, "cx");
        t.backspace();
        type_str(&mut t, "at");
        let s = t.stats();
        assert_eq!(s.correct_chars, 3);
        assert_eq!(s.incorrect_chars, 0);
        assert!(s.accuracy < 100.0);
    }

    #[test]
    fn backspace_word_removes_current_word() {
        let mut t = TypingTest::with_text(Mode::Words(3), "one two three");
        type_str(&mut t, "one tw");
        t.backspace_word();
        assert_eq!(t.typed.iter().collect::<String>(), "one ");
        t.backspace_word();
        assert_eq!(t.typed.iter().collect::<String>(), "");
    }

    #[test]
    fn words_progress_shrinks_per_word() {
        let mut t = TypingTest::with_text(Mode::Words(4), "aa bb cc dd");
        assert_eq!(t.fraction_left(), 1.0);
        type_str(&mut t, "aa b");
        assert_eq!(t.words_done(), 1);
        assert_eq!(t.fraction_left(), 0.75);
        type_str(&mut t, "b cc ");
        assert_eq!(t.fraction_left(), 0.25);
        type_str(&mut t, "dd");
        assert_eq!(t.fraction_left(), 0.0);
    }

    #[test]
    fn time_progress_is_full_before_start() {
        let t = TypingTest::with_text(Mode::Time(30), "aa bb");
        assert_eq!(t.fraction_left(), 1.0);
    }

    #[test]
    fn paused_time_is_not_counted() {
        use std::thread::sleep;
        let mut t = TypingTest::with_text(Mode::Time(30), "aa bb cc");
        t.pause(); // not started: nothing to pause
        assert!(!t.is_paused());
        t.type_char('a');
        sleep(Duration::from_millis(30));
        t.pause();
        let frozen = t.elapsed();
        sleep(Duration::from_millis(120));
        assert_eq!(t.elapsed(), frozen, "clock is frozen while paused");
        t.type_char('a'); // ignored while paused
        assert_eq!(t.typed.len(), 1);
        t.resume();
        let e = t.elapsed();
        assert!(e < Duration::from_millis(100), "pause excluded, got {e:?}");
        t.type_char('a');
        assert_eq!(t.typed.len(), 2);
    }

    #[test]
    fn timed_test_does_not_end_while_paused() {
        use std::thread::sleep;
        let mut t = TypingTest::with_text(Mode::Time(1), "aa bb cc");
        t.type_char('a');
        t.pause();
        sleep(Duration::from_millis(1100));
        t.tick();
        assert!(!t.is_finished());
        t.resume();
        t.tick();
        assert!(!t.is_finished(), "still has most of its second left");
    }

    #[test]
    fn timed_mode_keeps_adding_words() {
        let mut t = TypingTest::new(Mode::Time(60));
        let initial = t.target.len();
        let text: String = t.target.iter().collect();
        type_str(&mut t, &text);
        assert!(t.target.len() > initial);
        assert!(!t.is_finished());
    }
}
