//! Draws text as big letters made of block characters, for `/fontSize`.
//!
//! A terminal program can't change the terminal's real font, so instead each
//! letter is drawn from an 8×8 pixel bitmap (the `font8x8` crate) using block
//! characters:
//!
//! - size 1: normal text (not handled here)
//! - size 2: quadrant blocks (▘▝▖▗…), 2×2 pixels per cell → 4 cols × 4 rows per letter
//! - size 3+: half blocks (▀▄█), each pixel `size − 2` columns wide and
//!   `size − 2` half-rows tall → 8 cols × 4 rows at size 3, 16 × 8 at size 4, …
//!
//! Terminal cells are about twice as tall as they are wide, so half blocks
//! give roughly square pixels.

use font8x8::{BASIC_FONTS, UnicodeFonts};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub const MIN_SIZE: u8 = 1;
pub const MAX_SIZE: u8 = 10;

/// Quadrant characters indexed by bits: top-left 1, top-right 2, bottom-left 4, bottom-right 8.
const QUADRANTS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// Width and height in terminal cells of one letter at this size.
pub fn cell_size(size: u8) -> (usize, usize) {
    match size {
        0 | 1 => (1, 1),
        2 => (4, 4),
        s => {
            let scale = (s - 2) as usize;
            (8 * scale, 4 * scale)
        }
    }
}

fn bitmap(c: char) -> [u8; 8] {
    BASIC_FONTS
        .get(c)
        .or_else(|| BASIC_FONTS.get('?'))
        .unwrap_or([0; 8])
}

/// Is pixel (x, y) of this 8×8 bitmap set? Bit 0 of each row is the leftmost pixel.
fn pixel(bits: &[u8; 8], x: usize, y: usize) -> bool {
    x < 8 && y < 8 && (bits[y] >> x) & 1 == 1
}

/// The block character for cell (col, row) of a letter at this size.
fn cell_char(bits: &[u8; 8], size: u8, col: usize, row: usize) -> char {
    if size == 2 {
        let (x, y) = (col * 2, row * 2);
        let idx = pixel(bits, x, y) as usize
            | (pixel(bits, x + 1, y) as usize) << 1
            | (pixel(bits, x, y + 1) as usize) << 2
            | (pixel(bits, x + 1, y + 1) as usize) << 3;
        QUADRANTS[idx]
    } else {
        let scale = (size - 2) as usize;
        let x = col / scale;
        // Each row of cells is two half-rows; each pixel is `scale` half-rows tall.
        let top = pixel(bits, x, (row * 2) / scale);
        let bottom = pixel(bits, x, (row * 2 + 1) / scale);
        match (top, bottom) {
            (true, true) => '█',
            (true, false) => '▀',
            (false, true) => '▄',
            (false, false) => ' ',
        }
    }
}

/// Renders one line of styled characters as `cell_size(size).1` terminal rows.
pub fn render_line(chars: &[(char, Style)], size: u8) -> Vec<Line<'static>> {
    let (w, h) = cell_size(size);
    let glyphs: Vec<([u8; 8], Style)> = chars.iter().map(|&(c, s)| (bitmap(c), s)).collect();
    (0..h)
        .map(|row| {
            let spans: Vec<Span<'static>> = glyphs
                .iter()
                .map(|(bits, style)| {
                    let text: String = (0..w).map(|col| cell_char(bits, size, col, row)).collect();
                    Span::styled(text, *style)
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(c: char, size: u8) -> Vec<String> {
        render_line(&[(c, Style::new())], size)
            .into_iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn sizes_grow() {
        let mut prev = (1, 1);
        for s in 2..=MAX_SIZE {
            let cur = cell_size(s);
            assert!(
                cur.0 * cur.1 > prev.0 * prev.1,
                "size {s} bigger than {}",
                s - 1
            );
            prev = cur;
        }
    }

    #[test]
    fn rendered_letter_has_expected_dimensions() {
        for s in 2..=6 {
            let (w, h) = cell_size(s);
            let r = rows('a', s);
            assert_eq!(r.len(), h);
            assert!(r.iter().all(|line| line.chars().count() == w));
        }
    }

    #[test]
    fn space_is_blank_and_letters_are_not() {
        assert!(rows(' ', 3).iter().all(|l| l.trim().is_empty()));
        assert!(rows('w', 3).iter().any(|l| !l.trim().is_empty()));
        assert!(rows('w', 2).iter().any(|l| !l.trim().is_empty()));
    }

    #[test]
    fn letter_l_is_a_vertical_bar_shape() {
        // In font8x8, 'l' is a vertical stroke: several rows should be inked
        // in the same columns.
        let r = rows('l', 3);
        let inked_rows = r.iter().filter(|l| l.contains('█')).count();
        assert!(inked_rows >= 2, "{r:#?}");
    }
}
