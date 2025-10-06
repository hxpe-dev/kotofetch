use crate::config::RuntimeConfig;
use crate::quotes::BUILTIN_QUOTES;
use crate::quotes::Quote;
use crate::quotes::QuotesFile;
use console::{Color, Style};
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
    translation: Option<&str>,
    show_translation: bool,
    translation_style: Style,
    source: Option<&str>,
    show_source: bool,
    source_style: Style,
    centered: bool,
) {
    // Compute max natural width of content
    let mut max_width = 0;
    for line in &text_lines {
        max_width = max_width.max(UnicodeWidthStr::width(line.as_str()));
    }
    if show_translation {
        if let Some(t) = translation {
            max_width = max_width.max(UnicodeWidthStr::width(t));
        }
    }
    if show_source {
        if let Some(s) = source {
            max_width = max_width.max(UnicodeWidthStr::width(s));
        }
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

    // Translation
    if show_translation {
        if let Some(t) = translation {
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
                translation_style,
                inner_width,
                horizontal_padding,
                border,
                box_width,
                centered,
                &border_color,
            );
        }
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
                        format!("— {}", wline)
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

pub fn render(runtime: &RuntimeConfig, cli: &crate::cli::Cli) {
    // seed
    let seed = if cli.seed.unwrap_or(0) == 0 {
        rand::random::<u64>()
    } else {
        cli.seed.unwrap_or(runtime.seed)
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
        if let Some((_, content)) = BUILTIN_QUOTES
            .iter()
            .find(|&&(name, _)| name == file_name.to_str().unwrap())
        {
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
    let jap = simulate_font_size(&quote.japanese, &runtime.font_size);
    let jap_lines: Vec<String> = jap.lines().map(|s| s.to_string()).collect();
    let translation_style = color_from_hex(&runtime.translation_color);
    let show_source = runtime.source && quote.source.is_some();
    let source_style = Style::new().dim();

    let (translation, show_translation) = match runtime.show_translation {
        crate::config::TranslationMode::None => (None, false),
        crate::config::TranslationMode::English => {
            (quote.translation.as_deref(), quote.translation.is_some())
        }
        crate::config::TranslationMode::Romaji => (quote.romaji.as_deref(), quote.romaji.is_some()),
    };

    let jap_style = if runtime.bold {
        color_from_hex(&runtime.quote_color).bold()
    } else {
        color_from_hex(&runtime.quote_color)
    };

    let border_color = color_from_hex(&runtime.border_color);

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

            // estimate how many lines the box will take (content + borders + padding)
            let content_lines = {
                let mut count = jap_lines.len();
                if show_translation {
                    count += 1;
                }
                if show_source {
                    count += 1;
                }
                // Add vertical padding and border lines
                count += vertical * 2;
                if runtime.border {
                    count += 2;
                }
                count
            };

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
                translation,
                show_translation,
                translation_style.clone(),
                quote.source.as_deref(),
                show_source,
                source_style.clone(),
                runtime.centered,
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
    } else {
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
            translation,
            show_translation,
            translation_style,
            quote.source.as_deref(),
            show_source,
            source_style,
            runtime.centered,
        );
    }
}
