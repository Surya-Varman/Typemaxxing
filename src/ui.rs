//! Drawing with ratatui. The text area scales with the terminal: it uses most
//! of the width, shows as many lines as fit (double-spaced), and scrolls as
//! the cursor moves down.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};

use crate::app::{App, CommandMatch, match_command};
use crate::bigtext;
use crate::settings::{Kind, Settings, Theme};
use crate::typing::{Mode, TypingTest};

/// Show between this many lines of text, depending on terminal height.
const MIN_LINES: usize = 3;
const MAX_LINES: usize = 8;

thread_local! {
    /// The current theme's accent colour, set at the start of each `draw`.
    static ACCENT: std::cell::Cell<Color> = const { std::cell::Cell::new(Color::Indexed(141)) };
}

/// Accent colour for the cursor, bar, highlights and borders (see `/color`).
fn accent() -> Color {
    ACCENT.with(|a| a.get())
}

pub fn draw(frame: &mut Frame, app: &App) {
    ACCENT.with(|a| a.set(app.settings.theme.color()));
    let area = frame.area();
    if area.width < 24 || area.height < 10 {
        frame.render_widget(Paragraph::new("terminal too small"), area);
        return;
    }
    if app.test.is_finished() {
        draw_results(frame, app);
    } else {
        draw_test(frame, app);
    }
    draw_hints(frame, app);
    let other_overlay = app.show_help
        || app.confirm_quit_open
        || app.font_dialog_open
        || app.color_dialog_open
        || app.command.is_some();
    if app.test.is_paused() && !other_overlay {
        draw_paused(frame);
    }
    if app.show_help {
        draw_help(frame, app);
    }
    if app.confirm_quit_open {
        draw_quit_confirm(frame);
    }
    if app.font_dialog_open {
        draw_font_dialog(frame, app);
    }
    if app.color_dialog_open {
        draw_color_dialog(frame, app);
    }
}

/// Usable text width: the full terminal minus a ~10% margin on each side.
fn content_width(area: Rect) -> u16 {
    let margin = (area.width / 10).max(2);
    area.width - 2 * margin
}

/// Each text line takes two rows (text + a blank row), leaving room for the
/// header and the hints at the bottom.
fn visible_lines(area: Rect) -> usize {
    ((area.height.saturating_sub(9) / 2) as usize).clamp(MIN_LINES, MAX_LINES)
}

/// Splits text into lines of at most `width` chars, breaking after spaces.
/// Returns (start, end) char ranges; each space stays at the end of its line.
pub fn layout_lines(text: &[char], width: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut i = 0;
    while i < text.len() {
        // Find the end of the word starting at i (excluding its trailing space).
        let mut word_end = i;
        while word_end < text.len() && text[word_end] != ' ' {
            word_end += 1;
        }
        if word_end - line_start > width && i > line_start {
            lines.push((line_start, i));
            line_start = i;
        }
        // Step past the word and the space after it.
        i = (word_end + 1).min(text.len());
    }
    if line_start < text.len() {
        lines.push((line_start, text.len()));
    }
    lines
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn draw_hints(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let dim = Style::new().fg(Color::DarkGray);
    let key = Style::new().fg(Color::Gray);
    let accent = Style::new().fg(accent()).bold();

    let length = match app.settings.kind {
        Kind::Time => "change time limit",
        Kind::Words => "change length",
    };
    let previous = match app.settings.kind {
        Kind::Time => "previous time limit",
        Kind::Words => "previous length",
    };

    // (key, description) pairs, rendered as "key desc   key desc ..."
    let one_press = if app.settings.confirm_quit {
        "one-press quit: off"
    } else {
        "one-press quit: on"
    };
    let typed_command = app.command.as_deref().filter(|t| !t.is_empty());

    let items: Vec<(&str, &str)> = if app.confirm_quit_open {
        vec![("esc", "quit"), ("any other key", "keep going")]
    } else if app.font_dialog_open {
        vec![("↑", "bigger"), ("↓", "smaller"), ("enter", "done")]
    } else if app.color_dialog_open {
        vec![("← →", "choose color"), ("enter", "done")]
    } else if let Some(typed) = typed_command {
        // Partway through a long command: show it, then what keys do.
        return draw_command_line(frame, row_rect(area), typed);
    } else if app.command.is_some() {
        vec![
            ("m", "toggle mode"),
            ("s", length),
            ("S", previous),
            ("onePressQuit", one_press),
            ("fontSize", "text size"),
            ("color", "theme"),
            ("h", "help"),
        ]
    } else if app.show_help {
        vec![("/onePressQuit", one_press), ("esc", "close help")]
    } else if app.test.is_finished() {
        vec![
            ("enter", "next test"),
            ("/m", "toggle mode"),
            ("/s", length),
            ("/h", "help"),
            ("esc", "quit"),
        ]
    } else if app.test.is_paused() {
        vec![
            ("tab", "next test"),
            ("any other key", "resume"),
            ("/h", "help"),
            ("esc", "quit"),
        ]
    } else if app.test.has_started() {
        vec![("tab", "pause"), ("esc", "quit")]
    } else {
        vec![
            ("/m", "toggle mode"),
            ("/s", length),
            ("/h", "help"),
            ("tab", "new words"),
            ("esc", "quit"),
        ]
    };

    let mut spans = Vec::new();
    if app.command.is_some() && !app.confirm_quit_open {
        spans.push(Span::styled("/", accent));
        spans.push(Span::styled("  ", dim));
    }
    for (i, (k, d)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("     ", dim));
        }
        spans.push(Span::styled(*k, key));
        spans.push(Span::styled(format!(" {d}"), dim));
    }

    frame.render_widget(Paragraph::new(Line::from(spans).centered()), row_rect(area));
}

/// The hint bar's position: second-to-last row of the screen.
fn row_rect(area: Rect) -> Rect {
    Rect {
        x: area.x,
        y: area.y + area.height - 2,
        width: area.width,
        height: 1,
    }
}

/// While typing a long command: "/onePr" + dim "essQuit", or the typed text
/// in red with "unknown command" if it doesn't match anything.
fn draw_command_line(frame: &mut Frame, row: Rect, typed: &str) {
    let accent = Style::new().fg(accent()).bold();
    let dim = Style::new().fg(Color::DarkGray);
    let key = Style::new().fg(Color::Gray);
    let mut spans = vec![Span::styled("/", accent)];
    match match_command(typed) {
        CommandMatch::Partial(cmd) | CommandMatch::Exact(cmd) => {
            spans.push(Span::styled(cmd[..typed.len()].to_string(), accent));
            spans.push(Span::styled(cmd[typed.len()..].to_string(), dim));
        }
        CommandMatch::Unknown => {
            spans.push(Span::styled(
                typed.to_string(),
                Style::new().fg(Color::Red).bold(),
            ));
            spans.push(Span::styled(
                "   unknown command",
                Style::new().fg(Color::Red),
            ));
            spans.push(Span::styled("     backspace", key));
            spans.push(Span::styled(" fix", dim));
        }
    }
    spans.push(Span::styled("     esc", key));
    spans.push(Span::styled(" cancel", dim));
    frame.render_widget(Paragraph::new(Line::from(spans).centered()), row);
}

/// "time  words     15s  30s  60s  120s" with the current choices highlighted.
fn settings_bar(settings: &Settings) -> Vec<Span<'static>> {
    let on = Style::new().fg(accent()).bold();
    let off = Style::new().fg(Color::DarkGray);
    let pick = |selected: bool| if selected { on } else { off };

    let mut spans = vec![
        Span::styled("time", pick(settings.kind == Kind::Time)),
        Span::styled("  ", off),
        Span::styled("words", pick(settings.kind == Kind::Words)),
        Span::styled("     ", off),
    ];
    let (options, selected) = settings.length_options();
    for (i, label) in options.into_iter().enumerate() {
        spans.push(Span::styled(label, pick(selected == Some(i))));
        spans.push(Span::styled("  ", off));
    }
    if selected.is_none() {
        // A custom length from the command line, e.g. `-t 45`.
        let custom = match settings.mode() {
            Mode::Time(t) => format!("{t}s"),
            Mode::Words(w) => w.to_string(),
        };
        spans.push(Span::styled(custom, on));
    }
    spans
}

/// Small box above the hint bar (so the text behind stays visible while resizing):
///   Font size
///   ■ ■ ■ □ □ □ □ □ □ □   3 / 10
fn draw_font_dialog(frame: &mut Frame, app: &App) {
    let size = app.settings.font_size;
    let on = Style::new().fg(accent());
    let off = Style::new().fg(Color::DarkGray);
    let mut scale: Vec<Span> = (bigtext::MIN_SIZE..=bigtext::MAX_SIZE)
        .map(|s| {
            Span::styled(
                if s <= size { "■ " } else { "□ " },
                if s <= size { on } else { off },
            )
        })
        .collect();
    let label = if size == 1 {
        "  1 / 10  normal".to_string()
    } else {
        format!("  {size} / {}", bigtext::MAX_SIZE)
    };
    scale.push(Span::styled(label, Style::new().fg(Color::White).bold()));

    let lines = vec![
        Line::styled("Font size", Style::new().fg(Color::White).bold()).centered(),
        Line::default(),
        Line::from(scale).centered(),
    ];
    let area = frame.area();
    let w = 44.min(area.width);
    let h = lines.len() as u16 + 4;
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: (area.y + area.height).saturating_sub(h + 3),
        width: w,
        height: h,
    };
    let block = Block::bordered()
        .border_style(Style::new().fg(accent()))
        .padding(Padding::new(1, 1, 1, 1));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

/// Small box above the hint bar with the three themes; the current one is
/// marked and the whole screen already shows it.
fn draw_color_dialog(frame: &mut Frame, app: &App) {
    let mut choices: Vec<Span> = Vec::new();
    for (i, theme) in Theme::ALL.iter().enumerate() {
        if i > 0 {
            choices.push(Span::raw("      "));
        }
        let selected = *theme == app.settings.theme;
        let style = Style::new().fg(theme.color());
        choices.push(Span::styled(if selected { "● " } else { "○ " }, style));
        choices.push(Span::styled(
            theme.name(),
            if selected {
                style.bold().underlined()
            } else {
                style
            },
        ));
    }

    let lines = vec![
        Line::styled("Color theme", Style::new().fg(Color::White).bold()).centered(),
        Line::default(),
        Line::from(choices).centered(),
    ];
    let area = frame.area();
    let w = 44.min(area.width);
    let h = lines.len() as u16 + 4;
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: (area.y + area.height).saturating_sub(h + 3),
        width: w,
        height: h,
    };
    let block = Block::bordered()
        .border_style(Style::new().fg(accent()))
        .padding(Padding::new(1, 1, 1, 1));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_paused(frame: &mut Frame) {
    let key = Style::new().fg(accent()).bold();
    let lines = vec![
        Line::styled("Paused", Style::new().fg(Color::White).bold()).centered(),
        Line::default(),
        Line::from(vec![
            Span::styled("tab", key),
            Span::raw("  next test       "),
            Span::styled("any other key", key),
            Span::raw("  resume"),
        ])
        .centered(),
    ];
    let area = centered(frame.area(), 52, lines.len() as u16 + 4);
    let block = Block::bordered()
        .border_style(Style::new().fg(accent()))
        .padding(Padding::new(2, 2, 1, 1));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_quit_confirm(frame: &mut Frame) {
    let key = Style::new().fg(accent()).bold();
    let dim = Style::new().fg(Color::DarkGray);
    let lines = vec![
        Line::styled("Quit typemaxxing?", Style::new().fg(Color::White).bold()).centered(),
        Line::default(),
        Line::from(vec![
            Span::styled("esc", key),
            Span::raw("  quit       "),
            Span::styled("any other key", key),
            Span::raw("  keep going"),
        ])
        .centered(),
        Line::default(),
        Line::styled("skip this prompt with /onePressQuit", dim).centered(),
    ];
    let area = centered(frame.area(), 50, lines.len() as u16 + 4);
    let block = Block::bordered()
        .border_style(Style::new().fg(accent()))
        .padding(Padding::new(2, 2, 1, 1));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_help(frame: &mut Frame, app: &App) {
    let k = |s: &'static str| Span::styled(format!("{s:<22}"), Style::new().fg(accent()));
    let d = |s: &'static str| Span::raw(s);
    let heading = |s: &'static str| Line::styled(s, Style::new().fg(Color::Gray).bold());
    let row = |key, desc| Line::from(vec![k(key), d(desc)]);

    let lines = vec![
        heading("Commands (type / then the command, any time)"),
        row("/m", "toggle mode: time limit or word count"),
        row(
            "/s",
            "change time limit (15/30/60/120s) or length (10/25/50/100 words)",
        ),
        row("/S", "previous time limit or length"),
        row("/h", "show or hide this help"),
        Line::from(vec![
            k("/onePressQuit"),
            d("quit with a single esc, no confirmation: "),
            if app.settings.confirm_quit {
                Span::styled("off", Style::new().fg(Color::DarkGray).bold())
            } else {
                Span::styled("on", Style::new().fg(Color::Green).bold())
            },
        ]),
        Line::default(),
        heading("While typing"),
        row("backspace", "delete a character"),
        row("opt+backspace, ctrl+w", "delete a word"),
        row("tab", "pause; tab again for the next test"),
        row("enter", "next test (on the results screen)"),
        Line::from(vec![
            k("/fontSize"),
            d("bigger or smaller practice text with ↑ / ↓: "),
            Span::styled(
                format!("size {}", app.settings.font_size),
                Style::new().fg(accent()).bold(),
            ),
        ]),
        Line::from(vec![
            k("/color"),
            d("colour theme, purple, pink or yellow, with ← / →: "),
            Span::styled(app.settings.theme.name(), Style::new().fg(accent()).bold()),
        ]),
        row("esc", "quit (asks first unless /onePressQuit is on)"),
        row("ctrl+c", "quit immediately"),
        Line::default(),
        heading("From the command line"),
        row("tmx -t 45", "timed test of any length"),
        row("tmx -w 40", "any number of words"),
        row("tmx history", "past results and averages"),
        Line::default(),
        Line::styled(
            "Your settings are remembered for next time.",
            Style::new().fg(Color::DarkGray),
        ),
    ];

    let area = centered(frame.area(), 94, lines.len() as u16 + 4);
    let block = Block::bordered()
        .title(" help ")
        .border_style(Style::new().fg(Color::DarkGray))
        .padding(Padding::new(2, 2, 1, 1));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_test(frame: &mut Frame, app: &App) {
    let test = &app.test;
    let area = frame.area();
    let width = content_width(area);
    let n_lines = visible_lines(area);

    // Header: mode, timer, live WPM.
    let dim = Style::new().fg(Color::DarkGray);
    let accent = Style::new().fg(accent()).bold();
    let mut header = Vec::new();
    if test.has_started() {
        header.push(Span::styled(test.mode.label(), dim));
        let progress = match (test.mode, test.remaining()) {
            (Mode::Time(_), Some(rem)) => format!("{}", rem.as_secs_f64().ceil() as u64),
            (Mode::Words(n), _) => format!("{}/{n}", test.words_done()),
            _ => String::new(),
        };
        header.push(Span::styled("     ", dim));
        // No live WPM while typing; it's shown on the results screen.
        header.push(Span::styled(progress, accent));
        if test.is_paused() {
            header.push(Span::styled("     paused", accent));
        }
    } else {
        header = settings_bar(&app.settings);
        header.push(Span::styled("     start typing…", dim));
    }

    let mut rows: Vec<Line> = vec![
        Line::from(header),
        Line::default(),
        progress_bar(width, test.fraction_left()),
        Line::default(),
    ];

    // Rows left for the text: the screen minus the header block and the hints.
    let available = area.height.saturating_sub(9) as usize;
    let size = effective_font_size(app.settings.font_size, width as usize, available);
    if size < app.settings.font_size {
        rows[0].spans.push(Span::styled(
            format!(
                "     size {} doesn't fit this window, showing {size}",
                app.settings.font_size
            ),
            dim,
        ));
    }

    let text_rows = if size <= 1 {
        // Normal text: double-spaced lines.
        let lines = layout_lines(&test.target, width as usize);
        let first = first_visible_line(&lines, test.typed.len(), n_lines);
        for &(start, end) in lines.iter().skip(first).take(n_lines) {
            let spans: Vec<Span> = (start..end).map(|i| char_span(test, i)).collect();
            rows.push(Line::from(spans));
            rows.push(Line::default());
        }
        n_lines * 2
    } else {
        // Big block letters: each text line is `h` rows tall plus a blank row.
        let (w, h) = bigtext::cell_size(size);
        let per_line = (width as usize / w).max(1);
        let n_big = (available / (h + 1)).max(1);
        let lines = layout_lines(&test.target, per_line);
        let first = first_visible_line(&lines, test.typed.len(), n_big);
        for &(start, end) in lines.iter().skip(first).take(n_big) {
            let chars: Vec<(char, Style)> = (start..end).map(|i| big_char(test, i)).collect();
            rows.extend(bigtext::render_line(&chars, size));
            rows.push(Line::default());
        }
        n_big * (h + 1)
    };

    // Keep the block's height fixed so the text doesn't jump when lines run out.
    let height = (4 + text_rows) as u16;
    frame.render_widget(Paragraph::new(rows), centered(area, width, height));
}

/// A bar `width` cells wide whose filled part is `fraction` of it. The
/// used-up part stays visible as a dim track. Every cell is the same `█`
/// glyph (no background colours), so the bar is one even height throughout;
/// it shrinks a whole cell at a time, which is fine-grained enough at typical
/// widths (a 150-cell bar steps every 0.2s in a 30s test).
fn progress_bar(width: u16, fraction: f64) -> Line<'static> {
    let width = width as usize;
    let filled = (fraction.clamp(0.0, 1.0) * width as f64).round() as usize;
    Line::from(vec![
        Span::styled("█".repeat(filled), Style::new().fg(accent())),
        Span::styled("█".repeat(width - filled), Style::new().fg(Color::DarkGray)),
    ])
}

/// The largest size up to `requested` whose letters fit: at least 6 letters
/// per line and one line of text in the available height.
fn effective_font_size(requested: u8, width: usize, available_rows: usize) -> u8 {
    (2..=requested)
        .rev()
        .find(|&s| {
            let (w, h) = bigtext::cell_size(s);
            width / w >= 6 && available_rows > h
        })
        .unwrap_or(1)
}

/// Which line to show first: the cursor's line, with the previous line kept
/// above it when there's room for at least three lines.
fn first_visible_line(lines: &[(usize, usize)], cursor: usize, visible: usize) -> usize {
    let cursor_line = lines
        .iter()
        .position(|&(s, e)| cursor >= s && cursor < e)
        .unwrap_or(lines.len().saturating_sub(1));
    if visible >= 3 {
        cursor_line.saturating_sub(1)
    } else {
        cursor_line
    }
}

/// Character and colour for big block letters. Block letters can't use a
/// background highlight, so the cursor is a yellow letter, and a space at the
/// cursor or a missed space shows as "_".
fn big_char(test: &TypingTest, i: usize) -> (char, Style) {
    let expected = test.target[i];
    let cursor = test.typed.len();
    if i < cursor {
        if test.typed[i] == expected {
            (expected, Style::new().fg(Color::White))
        } else {
            let shown = if expected == ' ' { '_' } else { expected };
            (shown, Style::new().fg(Color::Red))
        }
    } else if i == cursor {
        let shown = if expected == ' ' { '_' } else { expected };
        (shown, Style::new().fg(accent()))
    } else {
        (expected, Style::new().fg(Color::DarkGray))
    }
}

fn char_span(test: &TypingTest, i: usize) -> Span<'static> {
    let expected = test.target[i];
    let cursor = test.typed.len();
    if i < cursor {
        if test.typed[i] == expected {
            Span::styled(expected.to_string(), Style::new().fg(Color::White))
        } else {
            // Show what should have been typed; a missed space shows as "_".
            let shown = if expected == ' ' { '_' } else { expected };
            Span::styled(
                shown.to_string(),
                Style::new()
                    .fg(Color::Red)
                    .add_modifier(Modifier::UNDERLINED),
            )
        }
    } else if i == cursor {
        Span::styled(
            expected.to_string(),
            Style::new().fg(Color::Black).bg(accent()),
        )
    } else {
        Span::styled(expected.to_string(), Style::new().fg(Color::DarkGray))
    }
}

fn draw_results(frame: &mut Frame, app: &App) {
    let s = app.test.stats();
    let dim = Style::new().fg(Color::DarkGray);
    let label = |t: &'static str| Span::styled(t, dim);
    let value = |v: String| Span::styled(v, Style::new().fg(Color::White).bold());

    // ── Left side: the details (the WPM itself is on the right) ──
    let title = if app.is_new_best() {
        Span::styled("new personal best!", Style::new().fg(Color::Green).bold())
    } else if let Some(best) = app.previous_best {
        Span::styled(format!("best {best:.0}"), dim)
    } else {
        Span::styled(format!("first result for {}", app.test.mode.label()), dim)
    };

    let save_line = match &app.save_status {
        Some(Ok(())) => Line::styled("saved to history (tmx history)", dim),
        Some(Err(e)) => Line::styled(
            format!("couldn't save result: {e}"),
            Style::new().fg(Color::Red),
        ),
        None => Line::default(),
    };

    let details = vec![
        Line::from(title),
        Line::default(),
        Line::from(vec![
            label("accuracy "),
            value(format!("{:.0}%", s.accuracy)),
            label("   raw "),
            value(format!("{:.0}", s.raw_wpm)),
            label("   chars "),
            value(format!("{}", s.correct_chars)),
            label("/"),
            Span::styled(
                format!("{}", s.incorrect_chars),
                Style::new().fg(Color::Red),
            ),
            label("   time "),
            value(format!("{:.0}s", s.seconds)),
        ]),
        Line::default(),
        Line::styled(app.test.mode.label(), dim),
        save_line,
    ];

    // ── Layout: two boxes side by side, centred ──
    let area = frame.area();
    let total_w = content_width(area);
    let gap = 2;
    // Left box is just wide enough for its text; the big number gets the rest.
    let details_w = details.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
    let left_w = (details_w + 4).min(total_w.saturating_sub(gap + 16));
    let right_w = total_w.saturating_sub(left_w + gap);
    // Inside each box: 1 border + 1 padding on each side.
    let inner = |w: u16| w.saturating_sub(4) as usize;

    let wpm_text = format!("{:.0}", s.wpm);
    let max_rows = area.height.saturating_sub(10) as usize;
    let (size, unit_size) = big_number_sizes(&wpm_text, inner(right_w), max_rows);
    let big: Vec<Line> = if size >= 2 {
        let digit_style = Style::new().fg(accent()).bold();
        let digits: Vec<(char, Style)> = wpm_text.chars().map(|c| (c, digit_style)).collect();
        let mut rows = bigtext::render_line(&digits, size);

        // "wpm" at about a third of the digits' height, bottom-aligned with
        // them so its baseline matches the number's. Rows above it get blank
        // space of the same width so the digits stay aligned when centred.
        let unit_style = Style::new().fg(accent());
        let unit_rows: Vec<Line> = if unit_size >= 2 {
            let letters: Vec<(char, Style)> = "wpm".chars().map(|c| (c, unit_style)).collect();
            bigtext::render_line(&letters, unit_size)
        } else {
            vec![Line::styled("wpm", unit_style)]
        };
        let unit_w = unit_rows.iter().map(|l| l.width()).max().unwrap_or(0);
        // Line the baselines up. In the 8×8 font the baseline is the bottom
        // of pixel row 6, which is 7 pixels down; one pixel is k half-rows
        // tall (k = size − 2, or 1 for quadrant size 2). So the label starts
        // (7·k_digits − 7·k_label) / 2 rows below the top of the digits.
        // Plain text goes on the digits' last row with ink.
        let half_rows_per_pixel = |s: u8| if s == 2 { 1 } else { (s - 2) as usize };
        let unit_top = if unit_size >= 2 {
            (7 * half_rows_per_pixel(size) - 7 * half_rows_per_pixel(unit_size)) / 2
        } else {
            rows.iter()
                .rposition(|l| l.spans.iter().any(|s| !s.content.trim().is_empty()))
                .unwrap_or(rows.len() - 1)
        };
        let unit_bottom = (unit_top + unit_rows.len()).min(rows.len());
        for (i, row) in rows.iter_mut().enumerate() {
            row.spans.push(Span::raw(" "));
            if (unit_top..unit_bottom).contains(&i) {
                let unit_line = &unit_rows[i - unit_top];
                row.spans.extend(unit_line.spans.iter().cloned());
                let pad = unit_w - unit_line.width();
                row.spans.push(Span::raw(" ".repeat(pad)));
            } else {
                row.spans.push(Span::raw(" ".repeat(unit_w)));
            }
        }
        rows.into_iter().map(|l| l.centered()).collect()
    } else {
        // Too little room for block digits: plain text instead.
        vec![
            Line::from(vec![
                Span::styled(wpm_text.clone(), Style::new().fg(accent()).bold()),
                Span::styled(" wpm", Style::new().fg(accent())),
            ])
            .centered(),
        ]
    };

    let height = (details.len().max(big.len()) + 4) as u16;
    let top = area.y + area.height.saturating_sub(height) / 2;
    let left_x = area.x + (area.width - total_w) / 2;
    let left = Rect {
        x: left_x,
        y: top,
        width: left_w,
        height,
    };
    let right = Rect {
        x: left_x + left_w + gap,
        y: top,
        width: right_w,
        height,
    };

    // No outlines, but the same spacing a border + padding would take, so
    // everything stays where it was.
    let boxed = || Block::new().padding(Padding::new(2, 2, 2, 2));

    // Vertically centre each box's content.
    let pad = |lines: Vec<Line<'static>>| {
        let inner_h = height.saturating_sub(4) as usize;
        let mut out = vec![Line::default(); inner_h.saturating_sub(lines.len()) / 2];
        out.extend(lines);
        out
    };
    frame.render_widget(Paragraph::new(pad(details)).block(boxed()), left);
    if right_w >= 12 {
        frame.render_widget(Paragraph::new(pad(big)).block(boxed()), right);
    }
}

/// Sizes for the results number and its "wpm" label.
///
/// Returns (digit size, unit size). The digits get the largest block size
/// (up to 5) that fits alongside the label; 1 means plain text. The label is
/// about a third of the digits' height: with half-block sizes (3+) a letter is
/// `4·k` rows tall where k = size − 2, so a third is k/3. A unit size of 1
/// means a plain-text "wpm".
fn big_number_sizes(text: &str, width: usize, rows: usize) -> (u8, u8) {
    let n = text.chars().count().max(1);
    for s in (2..=5).rev() {
        let (w, h) = bigtext::cell_size(s);
        let unit = unit_size_for(s);
        let unit_w = if unit >= 2 {
            3 * bigtext::cell_size(unit).0
        } else {
            3
        };
        if n * w + 1 + unit_w <= width && h <= rows {
            return (s, unit);
        }
    }
    (1, 1)
}

/// The "wpm" label size whose height is closest to a third of the digits'.
fn unit_size_for(digit_size: u8) -> u8 {
    let digit_rows = bigtext::cell_size(digit_size).1 as f64;
    let target = digit_rows / 3.0;
    // Candidates: plain text (1 row) or block sizes 3.. (4, 8, 12… rows).
    let mut best = (1u8, (1.0 - target).abs());
    for s in 3..digit_size {
        let r = bigtext::cell_size(s).1 as f64;
        if (r - target).abs() < best.1 {
            best = (s, (r - target).abs());
        }
    }
    // Size 2 is also 4 rows but narrower; prefer it only where size 3 isn't
    // allowed (digits at size 4, whose third is ~2.7 rows).
    if best.0 == 1 && digit_size == 4 {
        return 2;
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn render(text: &str, width: usize) -> Vec<String> {
        let t = chars(text);
        layout_lines(&t, width)
            .into_iter()
            .map(|(s, e)| t[s..e].iter().collect())
            .collect()
    }

    #[test]
    fn breaks_at_spaces() {
        assert_eq!(render("aaa bbb ccc ddd", 8), vec!["aaa bbb ", "ccc ddd"]);
    }

    #[test]
    fn lines_cover_all_text_without_gaps() {
        let text = "the quick brown fox jumps over the lazy dog again and again";
        for width in 5..30 {
            let t = chars(text);
            let lines = layout_lines(&t, width);
            assert_eq!(lines.first().unwrap().0, 0);
            assert_eq!(lines.last().unwrap().1, t.len());
            assert!(lines.windows(2).all(|w| w[0].1 == w[1].0));
            // No line's visible text (minus trailing space) exceeds width,
            // unless it's a single word that's longer than the width.
            for (s, e) in lines {
                let line: String = t[s..e].iter().collect();
                let trimmed = line.trim_end();
                assert!(trimmed.len() <= width || !trimmed.contains(' '));
            }
        }
    }

    fn bar_text(width: u16, fraction: f64) -> String {
        progress_bar(width, fraction)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn progress_bar_keeps_width_and_shrinks() {
        for f in [0.0, 0.01, 0.33, 0.5, 0.999, 1.0] {
            assert_eq!(bar_text(40, f).chars().count(), 40, "fraction {f}");
        }
        let full = progress_bar(10, 1.0);
        assert_eq!(full.spans[0].content, "█".repeat(10));
        let half = progress_bar(10, 0.5);
        assert_eq!(half.spans[0].content, "█".repeat(5));
        // No background colours anywhere, so every cell has the same height.
        assert!(half.spans.iter().all(|s| s.style.bg.is_none()));
    }

    #[test]
    fn font_size_falls_back_when_window_is_small() {
        // Plenty of room: the requested size is used.
        assert_eq!(effective_font_size(3, 200, 40), 3);
        // Size 6 letters are 32 cells wide: only 2 fit in 80 columns, so drop down.
        let s = effective_font_size(6, 80, 40);
        assert!((2..6).contains(&s), "{s}");
        // Tiny window: back to normal text.
        assert_eq!(effective_font_size(5, 20, 3), 1);
        assert_eq!(effective_font_size(1, 200, 40), 1);
    }

    #[test]
    fn big_wpm_shrinks_to_fit() {
        assert_eq!(big_number_sizes("85", 200, 40), (5, 3));
        // Size 5 digits are 24 wide; three need 72 columns plus the label.
        assert!(big_number_sizes("120", 80, 40).0 < 5);
        assert_eq!(big_number_sizes("85", 5, 40), (1, 1));
        assert_eq!(big_number_sizes("85", 200, 3), (1, 1));
    }

    #[test]
    fn wpm_label_is_about_a_third_of_the_number() {
        // 12-row digits → 4-row label.
        assert_eq!(bigtext::cell_size(unit_size_for(5)).1, 4);
        // 4-row digits → plain text.
        assert_eq!(unit_size_for(3), 1);
        assert_eq!(unit_size_for(2), 1);
    }

    #[test]
    fn long_word_gets_its_own_line() {
        assert_eq!(
            render("a verylongword b", 5),
            vec!["a ", "verylongword ", "b"]
        );
    }
}
