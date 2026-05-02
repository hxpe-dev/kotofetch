use crate::config::AnimationType;
use crate::config::RuntimeConfig;
use crate::quotes::BUILTIN_QUOTES;
use crate::quotes::Quote;
use crate::quotes::QuotesFile;
use console::{Color, Style, Term};
use rand::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{
    fs,
    io::{self, Write},
    thread,
};
use term_size;
use textwrap::wrap;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

fn simulate_font_size(s: &str, size: &str) -> String {
    match size {
        "small" => s.to_string(),
        "medium" => s
            .chars()
            .map(|c| {
                if c == '\n' {
                    "\n".to_string()
                } else {
                    format!("{c} ")
                }
            })
            .collect::<String>()
            .trim_end()
            .to_string(),
        "large" => s
            .chars()
            .map(|c| {
                if c == '\n' {
                    "\n".to_string()
                } else {
                    format!("{c}  ")
                }
            })
            .collect::<String>()
            .trim_end()
            .to_string(),
        _ => s.to_string(),
    }
}

enum Segment {
    Plain(String),
    Ruby { base: String, reading: String },
}

fn push_plain(segments: &mut Vec<Segment>, text: &str) {
    match segments.last_mut() {
        Some(Segment::Plain(s)) => s.push_str(text),
        _ => segments.push(Segment::Plain(text.to_string())),
    }
}

fn parse_ruby(s: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut kanji_run = String::new();
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if is_kanji(c) {
            kanji_run.push(c);
        } else if c == '(' && !kanji_run.is_empty() {
            let mut reading = String::new();
            let mut closed = false;
            for rc in chars.by_ref() {
                if rc == ')' {
                    closed = true;
                    break;
                }
                if rc == '\n' {
                    // Newline terminates an unclosed paren - treat as plain
                    reading.push(rc);
                    break;
                }
                reading.push(rc);
            }
            if closed {
                segments.push(Segment::Ruby {
                    base: kanji_run.clone(),
                    reading,
                });
            } else {
                let mut plain = kanji_run.clone();
                plain.push('(');
                plain.push_str(&reading);
                push_plain(&mut segments, &plain);
            }
            kanji_run.clear();
        } else {
            if !kanji_run.is_empty() {
                push_plain(&mut segments, &kanji_run.clone());
                kanji_run.clear();
            }
            push_plain(&mut segments, &c.to_string());
        }
    }

    if !kanji_run.is_empty() {
        push_plain(&mut segments, &kanji_run);
    }

    segments
}

fn format_ruby(segments: &[Segment]) -> (String, String) {
    let mut kanji_line = String::new();
    let mut furigana_line = String::new();

    for segment in segments {
        match segment {
            Segment::Plain(text) => {
                for c in text.chars() {
                    if c == '\n' {
                        kanji_line.push('\n');
                        furigana_line.push('\n');
                    } else {
                        let w = c.width().unwrap_or(1);
                        kanji_line.push(c);
                        furigana_line.push_str(&" ".repeat(w));
                    }
                }
            }
            Segment::Ruby { base, reading } => {
                let base_width = UnicodeWidthStr::width(base.as_str());
                let reading_width = UnicodeWidthStr::width(reading.as_str());
                let col = base_width.max(reading_width);
                let kanji_left = (col - base_width) / 2;
                let kanji_right = col - base_width - kanji_left;
                let reading_left = (col - reading_width) / 2;
                let reading_right = col - reading_width - reading_left;
                kanji_line.push_str(&" ".repeat(kanji_left));
                kanji_line.push_str(base);
                kanji_line.push_str(&" ".repeat(kanji_right));
                furigana_line.push_str(&" ".repeat(reading_left));
                furigana_line.push_str(reading);
                furigana_line.push_str(&" ".repeat(reading_right));
            }
        }
    }

    (
        kanji_line.trim_end().to_string(),
        furigana_line.trim_end().to_string(),
    )
}

fn strip_ruby(s: &str) -> String {
    parse_ruby(s)
        .iter()
        .map(|seg| match seg {
            Segment::Plain(t) => t.as_str(),
            Segment::Ruby { base, .. } => base.as_str(),
        })
        .collect()
}

fn has_ruby_markup(s: &str) -> bool {
    parse_ruby(s)
        .iter()
        .any(|seg| matches!(seg, Segment::Ruby { .. }))
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{20000}'..='\u{2A6DF}')
}

fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Map 24-bit to 6x6x6 cube (216 colors)
    let r = (r as f32 / 255.0 * 5.0).round() as u8;
    let g = (g as f32 / 255.0 * 5.0).round() as u8;
    let b = (b as f32 / 255.0 * 5.0).round() as u8;
    16 + (36 * r) + (6 * g) + b
}

fn color_from_hex(s: &str) -> Style {
    let lower = s.to_lowercase();

    // Named colors
    match lower.as_str() {
        "black" => return Style::new().fg(Color::Black),
        "red" => return Style::new().fg(Color::Red),
        "green" => return Style::new().fg(Color::Green),
        "yellow" => return Style::new().fg(Color::Yellow),
        "blue" => return Style::new().fg(Color::Blue),
        "magenta" => return Style::new().fg(Color::Magenta),
        "cyan" => return Style::new().fg(Color::Cyan),
        "white" => return Style::new().fg(Color::White),
        "dim" => return Style::new().dim(),
        _ => {}
    }

    // Hex color (#RRGGBB) -> map to nearest 256-color
    if let Some(stripped) = lower.strip_prefix('#') {
        if stripped.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&stripped[0..2], 16),
                u8::from_str_radix(&stripped[2..4], 16),
                u8::from_str_radix(&stripped[4..6], 16),
            ) {
                let idx = rgb_to_ansi256(r, g, b);
                return Style::new().fg(Color::Color256(idx));
            }
        }
    }

    // Fallback
    if lower.contains("gray") || lower.contains("grey") {
        Style::new().dim()
    } else {
        Style::new()
    }
}

// Center a whole line in the terminal if `centered` is true.
fn pad_to_center(line: &str, box_width: usize, centered: bool) -> String {
    if !centered {
        return line.to_string();
    }
    if let Some((term_width, _)) = term_size::dimensions() {
        if term_width > box_width {
            let pad = (term_width - box_width) / 2;
            return format!("{}{}", " ".repeat(pad), line);
        }
    }
    line.to_string()
}

// Center text within the inner box width if `centered` is true.
fn align_in_box(line: &str, inner_width: usize, centered: bool) -> String {
    let line_width = UnicodeWidthStr::width(line);
    if line_width >= inner_width {
        return line.to_string();
    }

    if centered {
        let total_pad = inner_width - line_width;
        let left = total_pad / 2;
        let right = total_pad - left;
        format!("{}{}{}", " ".repeat(left), line, " ".repeat(right))
    } else {
        // left align, just pad to the right
        format!("{}{}", line, " ".repeat(inner_width - line_width))
    }
}

// Create an empty line inside the box (used for spacing).
fn blank_line(
    inner_width: usize,
    horizontal_padding: usize,
    border: bool,
    border_color: &Style,
) -> String {
    if border {
        format!(
            "{}{}{}{}{}",
            border_color.apply_to("│"),
            " ".repeat(horizontal_padding),
            " ".repeat(inner_width),
            " ".repeat(horizontal_padding),
            border_color.apply_to("│")
        )
    } else {
        " ".repeat(horizontal_padding + inner_width)
    }
}

fn print_block(
    lines: &[String],
    style: Style,
    inner_width: usize,
    horizontal_padding: usize,
    border: bool,
    box_width: usize,
    centered: bool,
    border_color: &Style,
) {
    for line in lines {
        for wline in wrap(line, inner_width) {
            let content = align_in_box(wline.as_ref(), inner_width, centered);
            let line = if border {
                format!(
                    "{}{}{}{}{}",
                    border_color.apply_to("│"),
                    " ".repeat(horizontal_padding),
                    style.apply_to(content),
                    " ".repeat(horizontal_padding),
                    border_color.apply_to("│")
                )
            } else {
                format!(
                    "{}{}",
                    " ".repeat(horizontal_padding),
                    style.apply_to(content)
                )
            };
            println!("{}", pad_to_center(&line, box_width, centered));
        }
    }
}

fn print_boxed(
    text_lines: Vec<String>,
    jap_style: Style,
    horizontal_padding: usize,
    vertical_padding: usize,
    width: usize,
    border: bool,
    rounded_border: bool,
    border_color: Style,
    translations: &[&str],
    translation_style: Style,
    source: Option<&str>,
    show_source: bool,
    source_style: Style,
    centered: bool,
    furigana_lines: Vec<String>,
    furigana_above: bool,
) {
    // Compute max natural width of content
    let mut max_width = 0;
    for line in &text_lines {
        max_width = max_width.max(UnicodeWidthStr::width(line.as_str()));
    }
    for t in translations {
        max_width = max_width.max(UnicodeWidthStr::width(*t));
    }
    if show_source {
        if let Some(s) = source {
            max_width = max_width.max(UnicodeWidthStr::width(s));
        }
    }
    for line in &furigana_lines {
        max_width = max_width.max(UnicodeWidthStr::width(line.as_str()));
    }

    // Respect user specified width, width <= 0 means automatic
    let mut inner_width = if width > 0 { width } else { max_width };

    // Clamp inner width to terminal width minus borders/padding
    if let Some((term_width, _)) = term_size::dimensions() {
        let available =
            term_width.saturating_sub(horizontal_padding * 2 + if border { 2 } else { 0 });
        inner_width = inner_width.min(available);
    }

    let box_width = inner_width + horizontal_padding * 2 + if border { 2 } else { 0 };

    let (top_left, top_right, bottom_left, bottom_right) = if rounded_border {
        ('╭', '╮', '╰', '╯')
    } else {
        ('┌', '┐', '└', '┘')
    };
    let horiz = "─";

    // Top border
    if border {
        let line = format!(
            "{}{}{}",
            top_left,
            horiz.repeat(inner_width + horizontal_padding * 2),
            top_right
        );
        println!(
            "{}",
            border_color.apply_to(pad_to_center(&line, box_width, centered))
        );
    }

    // Vertical padding (top)
    for _ in 0..vertical_padding {
        println!(
            "{}",
            pad_to_center(
                &blank_line(inner_width, horizontal_padding, border, &border_color),
                box_width,
                centered
            )
        );
    }

    // Furigana above Japanese text
    // We compute the same left-margin the kanji line received and pre-apply it so that each reading lands directly under/above its kanji
    if furigana_above && !furigana_lines.is_empty() {
        for (idx, furi_line) in furigana_lines.iter().enumerate() {
            let kanji_w = text_lines
                .get(idx)
                .map(|l| UnicodeWidthStr::width(l.trim_end()))
                .unwrap_or(0);
            let left_pad = if centered && kanji_w < inner_width {
                (inner_width - kanji_w) / 2
            } else {
                0
            };
            let furi_w = UnicodeWidthStr::width(furi_line.as_str());
            let right_pad = inner_width.saturating_sub(left_pad + furi_w);
            let content = format!(
                "{}{}{}",
                " ".repeat(left_pad),
                furi_line,
                " ".repeat(right_pad)
            );
            let line = if border {
                format!(
                    "{}{}{}{}{}",
                    border_color.apply_to("│"),
                    " ".repeat(horizontal_padding),
                    translation_style.apply_to(&content),
                    " ".repeat(horizontal_padding),
                    border_color.apply_to("│")
                )
            } else {
                format!(
                    "{}{}",
                    " ".repeat(horizontal_padding),
                    translation_style.apply_to(&content)
                )
            };
            println!("{}", pad_to_center(&line, box_width, centered));
        }
        println!(
            "{}",
            pad_to_center(
                &blank_line(inner_width, horizontal_padding, border, &border_color),
                box_width,
                centered
            )
        );
    }

    // Japanese text
    print_block(
        &text_lines,
        jap_style,
        inner_width,
        horizontal_padding,
        border,
        box_width,
        centered,
        &border_color,
    );

    // Furigana below Japanese text
    if !furigana_above && !furigana_lines.is_empty() {
        println!(
            "{}",
            pad_to_center(
                &blank_line(inner_width, horizontal_padding, border, &border_color),
                box_width,
                centered
            )
        );
        for (idx, furi_line) in furigana_lines.iter().enumerate() {
            let kanji_w = text_lines
                .get(idx)
                .map(|l| UnicodeWidthStr::width(l.trim_end()))
                .unwrap_or(0);
            let left_pad = if centered && kanji_w < inner_width {
                (inner_width - kanji_w) / 2
            } else {
                0
            };
            let furi_w = UnicodeWidthStr::width(furi_line.as_str());
            let right_pad = inner_width.saturating_sub(left_pad + furi_w);
            let content = format!(
                "{}{}{}",
                " ".repeat(left_pad),
                furi_line,
                " ".repeat(right_pad)
            );
            let line = if border {
                format!(
                    "{}{}{}{}{}",
                    border_color.apply_to("│"),
                    " ".repeat(horizontal_padding),
                    translation_style.apply_to(&content),
                    " ".repeat(horizontal_padding),
                    border_color.apply_to("│")
                )
            } else {
                format!(
                    "{}{}",
                    " ".repeat(horizontal_padding),
                    translation_style.apply_to(&content)
                )
            };
            println!("{}", pad_to_center(&line, box_width, centered));
        }
    }

    // Translations
    for t in translations {
        println!(
            "{}",
            pad_to_center(
                &blank_line(inner_width, horizontal_padding, border, &border_color),
                box_width,
                centered
            )
        );
        print_block(
            &[t.to_string()],
            translation_style.clone(),
            inner_width,
            horizontal_padding,
            border,
            box_width,
            centered,
            &border_color,
        );
    }

    // Source
    if show_source {
        if let Some(s) = source {
            println!(
                "{}",
                pad_to_center(
                    &blank_line(inner_width, horizontal_padding, border, &border_color),
                    box_width,
                    centered
                )
            );
            let wrapped: Vec<String> = wrap(s, inner_width.saturating_sub(2))
                .into_iter()
                .enumerate()
                .map(|(i, wline)| {
                    if i == 0 {
                        format!("- {}", wline)
                    } else {
                        format!("  {}", wline)
                    }
                })
                .collect();
            print_block(
                &wrapped,
                source_style,
                inner_width,
                horizontal_padding,
                border,
                box_width,
                centered,
                &border_color,
            );
        }
    }

    // Vertical padding (bottom)
    for _ in 0..vertical_padding {
        println!(
            "{}",
            pad_to_center(
                &blank_line(inner_width, horizontal_padding, border, &border_color),
                box_width,
                centered
            )
        );
    }

    // Bottom border
    if border {
        let line = format!(
            "{}{}{}",
            bottom_left,
            horiz.repeat(inner_width + horizontal_padding * 2),
            bottom_right
        );
        println!(
            "{}",
            border_color.apply_to(pad_to_center(&line, box_width, centered))
        );
    }
}

fn clear_screen() {
    // ANSI escape: clear entire screen and move cursor to top-left
    print!("\x1B[2J\x1B[H");
    let _ = io::stdout().flush();
}

fn box_height(
    text_lines: &[String],
    translations: &[&str],
    furigana_lines: &[String],
    show_source: bool,
    vertical_padding: usize,
    border: bool,
    _furigana_above: bool,
) -> usize {
    let mut n = text_lines.len();
    if !furigana_lines.is_empty() {
        n += furigana_lines.len() + 1; // blank separator between japanese and furigana
    }
    n += translations.len() * 2; // spacer + content per translation
    if show_source {
        n += 2; // spacer + content line
    }
    n += vertical_padding * 2;
    if border {
        n += 2; // top and bottom border
    }
    n
}

struct CursorGuard;

impl CursorGuard {
    fn new() -> Self {
        print!("\x1B[?25l");
        let _ = io::stdout().flush();
        CursorGuard
    }
}

impl Drop for CursorGuard {
    fn drop(&mut self) {
        print!("\x1B[?25h");
        let _ = io::stdout().flush();
    }
}

// Collect all visible (non-newline) chars from lines in reading order, recording (line, byte_offset, char, width).
fn collect_chars(lines: &[String]) -> Vec<(usize, usize, char, usize)> {
    let mut out = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        for (bi, c) in line.char_indices() {
            if c != '\n' {
                out.push((li, bi, c, c.width().unwrap_or(1)));
            }
        }
    }
    out
}

fn anim_typewriter(real: &[String], reveal: usize) -> Vec<String> {
    let mut remaining = reveal;
    real.iter()
        .map(|line| {
            let total_w = UnicodeWidthStr::width(line.as_str());
            let chars: Vec<char> = line.chars().collect();
            let to_show = remaining.min(chars.len());
            remaining = remaining.saturating_sub(chars.len());
            let visible: String = chars[..to_show].iter().collect();
            let shown_w: usize = visible.chars().map(|c| c.width().unwrap_or(1)).sum();
            format!("{}{}", visible, " ".repeat(total_w.saturating_sub(shown_w)))
        })
        .collect()
}

fn anim_slide(real: &[String], frame: usize, total_frames: usize) -> Vec<String> {
    real.iter()
        .map(|line| {
            let w = UnicodeWidthStr::width(line.as_str());
            let offset = w.saturating_sub(w * frame / total_frames.max(1));
            if offset == 0 {
                line.clone()
            } else {
                // take the last (w - offset) chars and pad left with spaces
                let chars: Vec<char> = line.chars().collect();
                let visible_chars = chars.len().saturating_sub(offset);
                let visible: String = chars[chars.len() - visible_chars..].iter().collect();
                let visible_w = UnicodeWidthStr::width(visible.as_str());
                format!("{}{}", " ".repeat(w.saturating_sub(visible_w)), visible)
            }
        })
        .collect()
}

// global_offset: position of the first char of `real` in the combined jap+trans sequence
// n_total: total chars across all animated text
// pool: replacement chars (kana for jpanese, ASCII for translations)
fn anim_scramble_lines(
    real: &[String],
    global_offset: usize,
    n_total: usize,
    frame: usize,
    total_frames: usize,
    rng: &mut StdRng,
    pool: &[char],
) -> Vec<String> {
    const SCRAMBLE_STEPS: usize = 6;
    let chars = collect_chars(real);
    if chars.is_empty() || pool.is_empty() {
        return real.to_vec();
    }
    let n_total = n_total.max(1);

    let mut per_line: Vec<Vec<Option<char>>> =
        real.iter().map(|l| l.chars().map(Some).collect()).collect();

    for (k, (li, _, real_c, _)) in chars.iter().enumerate() {
        let global_k = global_offset + k;
        let settle_frame = total_frames.saturating_sub(SCRAMBLE_STEPS) * global_k / n_total;
        let char_frame = if real_c.is_whitespace() {
            *real_c
        } else if frame < settle_frame + SCRAMBLE_STEPS {
            pool[rng.random_range(0..pool.len())]
        } else {
            *real_c
        };
        let pos_in_line = chars[..k].iter().filter(|(l2, _, _, _)| *l2 == *li).count();
        if let Some(slot) = per_line[*li].get_mut(pos_in_line) {
            *slot = Some(char_frame);
        }
    }

    per_line
        .iter()
        .zip(real.iter())
        .map(|(scrambled, orig)| {
            let s: String = scrambled.iter().filter_map(|c| *c).collect();
            let w = UnicodeWidthStr::width(orig.as_str());
            let sw = UnicodeWidthStr::width(s.as_str());
            if sw < w {
                format!("{}{}", s, " ".repeat(w - sw))
            } else {
                s
            }
        })
        .collect()
}

fn play_animation(
    animation: &AnimationType,
    duration_ms: u64,
    seed: u64,
    real_jap_lines: &[String],
    real_furigana_lines: &[String],
    real_translations: &[&str],
    real_source: Option<&str>,
    show_source: bool,
    jap_style: Style,
    translation_style: Style,
    source_style: Style,
    horizontal_padding: usize,
    vertical_padding: usize,
    width: usize,
    border: bool,
    rounded_border: bool,
    border_color: Style,
    centered: bool,
    furigana_above: bool,
) {
    let height = box_height(
        real_jap_lines,
        real_translations,
        real_furigana_lines,
        show_source,
        vertical_padding,
        border,
        furigana_above,
    );

    let n_furi_chars: usize = real_furigana_lines.iter().map(|l| l.chars().count()).sum();
    let blank_source_owned: String = real_source
        .map(|s| " ".repeat(UnicodeWidthStr::width(s)))
        .unwrap_or_default();
    let blank_source: Option<&str> = real_source.map(|_| blank_source_owned.as_str());

    let n_jap_chars: usize = real_jap_lines.iter().map(|l| l.chars().count()).sum();
    let n_trans_chars: usize = real_translations.iter().map(|t| t.chars().count()).sum();
    let n_total = (n_jap_chars + n_trans_chars).max(1);

    let frame_count: usize = match animation {
        AnimationType::None => 0,
        AnimationType::Typewriter => n_total,
        AnimationType::Scramble => 40,
        AnimationType::Slide => 30,
    };

    let frame_delay_ms = if frame_count == 0 {
        0
    } else {
        (duration_ms / frame_count as u64).clamp(16, 200)
    };

    let trans_strings: Vec<String> = real_translations.iter().map(|&s| s.to_string()).collect();

    let jap_pool: Vec<char> = (0x3041u32..=0x3096)
        .chain(0x30A1..=0x30F6)
        .filter_map(char::from_u32)
        .collect();
    let ascii_pool: Vec<char> = (b'a'..=b'z')
        .chain(b'A'..=b'Z')
        .chain(b'0'..=b'9')
        .map(|b| b as char)
        .collect();

    let mut rng = StdRng::seed_from_u64(seed);

    let _guard = CursorGuard::new();

    for i in 0..=frame_count {
        let is_final = i == frame_count;
        let n_jap_reveal = i.min(n_jap_chars);
        let n_trans_reveal = i.saturating_sub(n_jap_chars);

        let frame_jap = match animation {
            AnimationType::None => real_jap_lines.to_vec(),
            AnimationType::Typewriter => anim_typewriter(real_jap_lines, n_jap_reveal),
            AnimationType::Slide => anim_slide(real_jap_lines, i, frame_count),
            AnimationType::Scramble => anim_scramble_lines(
                real_jap_lines,
                0,
                n_total,
                i,
                frame_count,
                &mut rng,
                &jap_pool,
            ),
        };

        let frame_trans_owned: Vec<String> = if is_final {
            trans_strings.clone()
        } else {
            match animation {
                AnimationType::None => trans_strings.clone(),
                AnimationType::Typewriter => anim_typewriter(&trans_strings, n_trans_reveal),
                AnimationType::Slide => anim_slide(&trans_strings, i, frame_count),
                AnimationType::Scramble => anim_scramble_lines(
                    &trans_strings,
                    n_jap_chars,
                    n_total,
                    i,
                    frame_count,
                    &mut rng,
                    &ascii_pool,
                ),
            }
        };
        let frame_trans_refs: Vec<&str> = frame_trans_owned.iter().map(|s| s.as_str()).collect();

        let furi: Vec<String> = if real_furigana_lines.is_empty() || is_final {
            real_furigana_lines.to_vec()
        } else {
            match animation {
                AnimationType::Typewriter => {
                    let reveal = n_furi_chars * n_jap_reveal / n_jap_chars.max(1);
                    anim_typewriter(real_furigana_lines, reveal)
                }
                AnimationType::Slide => anim_slide(real_furigana_lines, i, frame_count),
                AnimationType::Scramble => anim_scramble_lines(
                    real_furigana_lines,
                    0,
                    n_furi_chars.max(1),
                    i,
                    frame_count,
                    &mut rng,
                    &jap_pool,
                ),
                AnimationType::None => real_furigana_lines.to_vec(),
            }
        };
        let src = if is_final { real_source } else { blank_source };

        if i > 0 {
            print!("\x1B[{}A", height);
        }

        print_boxed(
            frame_jap,
            jap_style.clone(),
            horizontal_padding,
            vertical_padding,
            width,
            border,
            rounded_border,
            border_color.clone(),
            &frame_trans_refs,
            translation_style.clone(),
            src,
            show_source,
            source_style.clone(),
            centered,
            furi,
            furigana_above,
        );

        let _ = io::stdout().flush();

        if i < frame_count {
            thread::sleep(Duration::from_millis(frame_delay_ms));
        }
    }
}

pub fn render(runtime: &RuntimeConfig, cli: &crate::cli::Cli) {
    // seed
    let seed = if runtime.seed == 0 {
        rand::random::<u64>()
    } else {
        runtime.seed
    };
    let mut rng = StdRng::seed_from_u64(seed);

    // collect quotes
    let mut pool = Vec::new();
    for mode_file in &runtime.modes {
        // Ensure the file has .toml extension
        let mut file_name = mode_file.clone();
        if file_name.extension().is_none() {
            file_name.set_extension("toml");
        }

        // Look in ~/.config/kotofetch/quotes first
        let mut path = dirs::config_dir().unwrap_or_default();
        path.push("kotofetch/quotes");
        path.push(&file_name);

        if path.exists() {
            if let Ok(s) = fs::read_to_string(&path) {
                match toml::from_str::<QuotesFile>(&s) {
                    Ok(parsed) => pool.extend(parsed.quotes),
                    Err(e) => eprintln!("Failed to parse {}: {e}", path.display()),
                }
            } else {
                eprintln!("Failed to read file: {}", path.display());
            }
            continue; // skip built-in if config exists
        }

        // fallback to built-in
        let file_str = file_name.to_str().unwrap_or_default();
        if let Some((_, content)) = BUILTIN_QUOTES.iter().find(|&&(name, _)| name == file_str) {
            match toml::from_str::<QuotesFile>(content) {
                Ok(parsed) => pool.extend(parsed.quotes),
                Err(e) => eprintln!("Failed to parse built-in {}: {e}", file_str),
            }
        } else {
            eprintln!(
                "Warning: mode file not found in config or built-in: {}",
                file_str
            );
        }
    }

    if pool.is_empty() {
        pool.push(Quote {
            japanese: "(no quote found)".to_string(),
            translation: None,
            romaji: None,
            source: None,
        });
    }

    // pick quote
    let quote = if let Some(i) = cli.index {
        pool.get(i).cloned()
    } else {
        pool.choose(&mut rng).cloned()
    }
    .unwrap();

    // render
    let translation_style = color_from_hex(&runtime.translation_color);
    let show_source = runtime.source && quote.source.is_some();
    let source_style = Style::new().dim();

    let mut translations: Vec<&str> = Vec::new();
    let mut show_furigana = false;

    for mode in &runtime.show_translation {
        match mode {
            crate::config::TranslationMode::None => {}
            crate::config::TranslationMode::English => {
                if let Some(t) = quote.translation.as_deref() {
                    translations.push(t);
                }
            }
            crate::config::TranslationMode::Romaji => {
                if let Some(r) = quote.romaji.as_deref() {
                    translations.push(r);
                }
            }
            crate::config::TranslationMode::Furigana => {
                show_furigana = has_ruby_markup(&quote.japanese);
            }
        }
    }

    let furigana_above = matches!(
        runtime.furigana_position,
        crate::config::FuriganaPosition::Above
    );

    let jap_style = if runtime.bold {
        color_from_hex(&runtime.quote_color).bold()
    } else {
        color_from_hex(&runtime.quote_color)
    };

    let border_color = color_from_hex(&runtime.border_color);

    let (jap_lines, furigana_lines): (Vec<String>, Vec<String>) = if show_furigana {
        let segments = parse_ruby(&quote.japanese);
        let (kanji, reading) = format_ruby(&segments);
        let kanji_lines: Vec<String> = kanji.lines().map(|s| s.to_string()).collect();
        let reading_lines: Vec<String> = reading.lines().map(|s| s.to_string()).collect();
        (kanji_lines, reading_lines)
    } else {
        let jap = simulate_font_size(&strip_ruby(&quote.japanese), &runtime.font_size);
        let lines: Vec<String> = jap.lines().map(|s| s.to_string()).collect();
        (lines, vec![])
    };

    let animated = runtime.animation != AnimationType::None && Term::stdout().is_term();

    if animated {
        play_animation(
            &runtime.animation,
            runtime.animation_duration_ms,
            seed,
            &jap_lines,
            &furigana_lines,
            &translations,
            quote.source.as_deref(),
            show_source,
            jap_style.clone(),
            translation_style.clone(),
            source_style.clone(),
            runtime.horizontal_padding,
            runtime.vertical_padding,
            runtime.width,
            runtime.border,
            runtime.rounded_border,
            border_color.clone(),
            runtime.centered,
            furigana_above,
        );
    }

    if runtime.dynamic {
        // Dynamic recentering mode
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        // Handle Ctrl+C gracefully
        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");

        // Hide cursor
        print!("\x1B[?25l");
        io::stdout().flush().unwrap();

        let mut last_size = term_size::dimensions();

        while running.load(Ordering::SeqCst) {
            clear_screen();

            let (_, term_h) = term_size::dimensions().unwrap_or((80, 24));

            let mut vertical = 0;
            let mut horizontal = 0;

            if runtime.border {
                vertical = runtime.vertical_padding;
                horizontal = runtime.horizontal_padding;
            }

            let content_lines = box_height(
                &jap_lines,
                &translations,
                &furigana_lines,
                show_source,
                vertical,
                runtime.border,
                furigana_above,
            );

            // Compute top blank lines to center vertically
            let top_blank = if term_h > content_lines {
                (term_h - content_lines) / 2
            } else {
                1
            };

            // Print top spacing
            for _ in 0..top_blank {
                println!();
            }

            // Render centered block
            print_boxed(
                jap_lines.clone(),
                jap_style.clone(),
                horizontal,
                vertical,
                runtime.width,
                runtime.border,
                runtime.rounded_border,
                border_color.clone(),
                &translations,
                translation_style.clone(),
                quote.source.as_deref(),
                show_source,
                source_style.clone(),
                runtime.centered,
                furigana_lines.clone(),
                furigana_above,
            );

            io::stdout().flush().unwrap();

            // Sleep before checking for resize or exit
            thread::sleep(Duration::from_millis(200));

            let current_size = term_size::dimensions();
            if current_size != last_size {
                last_size = current_size;
                clear_screen(); // redraw on resize
            }
        }

        // Show cursor again before exiting
        print!("\x1B[?25h");
        io::stdout().flush().unwrap();

        clear_screen(); // clean terminal on exit
    } else if !animated {
        // Normal static render
        print_boxed(
            jap_lines,
            jap_style,
            runtime.horizontal_padding,
            runtime.vertical_padding,
            runtime.width,
            runtime.border,
            runtime.rounded_border,
            border_color,
            &translations,
            translation_style,
            quote.source.as_deref(),
            show_source,
            source_style,
            runtime.centered,
            furigana_lines,
            furigana_above,
        );
    }
}
