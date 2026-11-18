//! WPM and accuracy calculations.
//!
//! - WPM works like Monkeytype: only words typed completely correctly count,
//!   plus the space after each one, using the standard "5 characters = 1 word".
//!   It's judged on the final text, so a mistake fixed with backspace before
//!   the end costs nothing. A word left with any wrong letter earns nothing.
//! - Raw WPM counts every character currently typed, right or wrong.
//! - Accuracy counts every keystroke, so a mistake you fixed with
//!   backspace still lowers it.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub wpm: f64,
    /// Characters credited towards WPM (correct words and their spaces).
    pub counted_chars: usize,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub correct_chars: usize,
    pub incorrect_chars: usize,
    pub seconds: f64,
}

/// Characters in correctly typed words, plus the space after each.
///
/// A word counts only if every letter matches and the space after it was
/// typed correctly (a letter typed where the space belongs makes the word
/// wrong, like an extra letter). The word the cursor is still in counts if
/// everything typed of it so far is correct, so a timed test cut off
/// mid-word isn't penalised.
pub fn counted_chars(target: &[char], typed: &[char]) -> usize {
    let mut counted = 0;
    let mut start = 0;
    while start < target.len() && start < typed.len() {
        // The word is target[start..end]; target[end] is its space (if any).
        let mut end = start;
        while end < target.len() && target[end] != ' ' {
            end += 1;
        }
        let typed_end = end.min(typed.len());
        let letters_ok = typed[start..typed_end] == target[start..typed_end];

        if typed.len() <= end {
            // Still inside this word (or just finished its last letter).
            if letters_ok {
                counted += typed.len() - start;
            }
            break;
        }
        // Past the word: it needs all letters right and the space typed right.
        if letters_ok && typed[end] == ' ' {
            counted += end - start + 1;
        }
        start = end + 1;
    }
    counted
}

pub fn compute(
    target: &[char],
    typed: &[char],
    keystrokes: usize,
    errors: usize,
    elapsed: Duration,
) -> Stats {
    let correct_chars = typed
        .iter()
        .zip(target.iter())
        .filter(|(a, b)| a == b)
        .count();
    let incorrect_chars = typed.len() - correct_chars;
    let counted = counted_chars(target, typed);
    let seconds = elapsed.as_secs_f64();
    let minutes = seconds / 60.0;

    let per_minute = |chars: usize| {
        if seconds < 0.5 {
            0.0 // avoid absurd numbers in the first instant of a test
        } else {
            (chars as f64 / 5.0) / minutes
        }
    };

    let accuracy = if keystrokes == 0 {
        100.0
    } else {
        (keystrokes.saturating_sub(errors)) as f64 / keystrokes as f64 * 100.0
    };

    Stats {
        wpm: per_minute(counted),
        counted_chars: counted,
        raw_wpm: per_minute(typed.len()),
        accuracy,
        correct_chars,
        incorrect_chars,
        seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn perfect_minute() {
        // 50 correct chars in 60s = 10 WPM
        let t = chars(&"a".repeat(50));
        let s = compute(&t, &t, 50, 0, Duration::from_secs(60));
        assert!((s.wpm - 10.0).abs() < 1e-9);
        assert!((s.raw_wpm - 10.0).abs() < 1e-9);
        assert_eq!(s.accuracy, 100.0);
    }

    #[test]
    fn mistakes_lower_wpm_but_not_raw() {
        let target = chars("hello world");
        let typed = chars("hxllo world");
        let s = compute(&target, &typed, 11, 1, Duration::from_secs(30));
        assert_eq!(s.correct_chars, 10);
        assert_eq!(s.incorrect_chars, 1);
        assert_eq!(
            s.counted_chars, 5,
            "only 'world' counts; 'hxllo ' earns nothing"
        );
        assert!(s.wpm < s.raw_wpm);
        assert!((s.accuracy - 100.0 * 10.0 / 11.0).abs() < 1e-9);
    }

    fn counted(target: &str, typed: &str) -> usize {
        counted_chars(&chars(target), &chars(typed))
    }

    #[test]
    fn one_wrong_letter_loses_the_whole_word() {
        assert_eq!(counted("the cat sat", "the cxt sat"), 4 + 3); // "the " + "sat"
    }

    #[test]
    fn fixed_mistakes_cost_nothing() {
        // Judged on the final text: a typo fixed with backspace leaves no trace.
        assert_eq!(counted("the cat sat", "the cat sat"), 11);
    }

    #[test]
    fn letter_typed_instead_of_space_spoils_the_word() {
        // "ab" then "x" where the space belongs: like an extra letter, so "ab" is wrong.
        assert_eq!(counted("ab cd", "abxcd"), 2);
    }

    #[test]
    fn word_in_progress_counts_if_correct_so_far() {
        assert_eq!(counted("hello world", "hello wor"), 6 + 3);
        assert_eq!(counted("hello world", "hello wxr"), 6);
        assert_eq!(counted("hello world", "hello "), 6);
        assert_eq!(counted("hello world", "hell"), 4);
        assert_eq!(counted("hello world", ""), 0);
    }

    #[test]
    fn last_word_without_trailing_space_counts() {
        assert_eq!(counted("hi there", "hi there"), 8);
        assert_eq!(counted("hi there", "hi therx"), 3);
    }

    #[test]
    fn zero_time_is_zero_wpm() {
        let t = chars("abc");
        let s = compute(&t, &t, 3, 0, Duration::ZERO);
        assert_eq!(s.wpm, 0.0);
    }
}
