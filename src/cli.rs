use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    // Path to config file (TOML). Defaults to ~/.config/kotofetch/config.toml
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    // Horizontal padding
    #[arg(long)]
    pub horizontal_padding: Option<usize>,

    // Vertical padding
    #[arg(long)]
    pub vertical_padding: Option<usize>,

    // Override width (0 = automatic)
    #[arg(long)]
    pub width: Option<usize>,

    // Choose translation mode: none, english or romaji
    #[arg(long, value_enum)]
    pub translation: Option<TranslationMode>,

    // Translation color (hex like #888888 or named)
    #[arg(long)]
    pub translation_color: Option<String>,

    // Quote color (hex like #888888 or named)
    #[arg(long)]
    pub quote_color: Option<String>,

    // Make Japanese text bold
    #[arg(long)]
    pub bold: Option<bool>,

    // Draw a border around the quote
    #[arg(long)]
    pub border: Option<bool>,

    // Is the border rounded?
    #[arg(long)]
    pub rounded_border: Option<bool>,

    // Border color (hex like #888888 or named)
    #[arg(long)]
    pub border_color: Option<String>,

    // Show quote source
    #[arg(long)]
    pub source: Option<bool>,

    // Quote options
    #[arg(long, value_delimiter = ',', num_args = 1.., required = false)]
    pub modes: Option<Vec<PathBuf>>,

    // Choose a specific quote by index (0-based) for reproducible output
    #[arg(long)]
    pub index: Option<usize>,

    // Seed for random selection (0 = random by time)
    #[arg(long)]
    pub seed: Option<u64>,

    // Center text
    #[arg(long)]
    pub centered: Option<bool>,

    // Dynamic re-centering text
    #[arg(long)]
    pub dynamic: Option<bool>,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum TranslationMode {
    None,
    English,
    Romaji,
}
