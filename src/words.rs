//! Built-in word list and random word generation. Everything is bundled into
//! the binary, so the test never touches the network or the filesystem.

use rand::seq::IndexedRandom;

const ENGLISH: &str = include_str!("../assets/english.txt");

fn word_list() -> Vec<&'static str> {
    ENGLISH.split_whitespace().collect()
}

/// Returns `count` random words, avoiding the same word twice in a row.
pub fn generate(count: usize) -> Vec<&'static str> {
    let list = word_list();
    let mut rng = rand::rng();
    let mut out: Vec<&'static str> = Vec::with_capacity(count);
    while out.len() < count {
        let w = *list.choose(&mut rng).expect("word list is not empty");
        if out.last() != Some(&w) {
            out.push(w);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_reasonable() {
        let list = word_list();
        assert!(list.len() >= 150);
        assert!(
            list.iter()
                .all(|w| w.chars().all(|c| c.is_ascii_lowercase()))
        );
    }

    #[test]
    fn generates_requested_count_without_repeats() {
        let words = generate(500);
        assert_eq!(words.len(), 500);
        assert!(words.windows(2).all(|p| p[0] != p[1]));
    }
}
