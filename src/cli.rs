use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "kotofetch", author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to config file (TOML). Defaults to ~/.config/kotofetch/config.toml
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Horizontal padding
    #[arg(long)]
    pub horizontal_padding: Option<usize>,

    /// Vertical padding
    #[arg(long)]
    pub vertical_padding: Option<usize>,

    /// Override width (0 = automatic)
    #[arg(long)]
    pub width: Option<usize>,

    /// Choose translation modes (comma-separated): none, english, romaji, furigana
    #[arg(long, value_enum, value_delimiter = ',', num_args = 1..)]
    pub translation: Option<Vec<TranslationMode>>,

    /// Translation color (hex like #888888 or named)
    #[arg(long)]
    pub translation_color: Option<String>,

    /// Quote color (hex like #888888 or named)
    #[arg(long)]
    pub quote_color: Option<String>,

    /// Make Japanese text bold
    #[arg(long)]
    pub bold: Option<bool>,

    /// Draw a border around the quote
    #[arg(long)]
    pub border: Option<bool>,

    /// Is the border rounded?
    #[arg(long)]
    pub rounded_border: Option<bool>,

    /// Border color (hex like #888888 or named)
    #[arg(long)]
    pub border_color: Option<String>,

    /// Show quote source
    #[arg(long)]
    pub source: Option<bool>,

    /// Quote options
    #[arg(long, value_delimiter = ',', num_args = 1.., required = false)]
    pub modes: Option<Vec<PathBuf>>,

    /// Choose a specific quote by index (0-based) for reproducible output
    #[arg(long)]
    pub index: Option<usize>,

    /// Seed for random selection (0 = random by time)
    #[arg(long)]
    pub seed: Option<u64>,

    /// Center text
    #[arg(long)]
    pub centered: Option<bool>,

    /// Dynamic re-centering text
    #[arg(long)]
    pub dynamic: Option<bool>,

    /// Show furigana above or below the Japanese text
    #[arg(long, value_enum)]
    pub furigana_position: Option<FuriganaPosition>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Import quotes from external sources
    Init {
        #[command(subcommand)]
        source: InitSource,
    },
    /// Output the completion script
    Completion {
        /// Select the shell
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum InitSource {
    /// Import Anki decks via AnkiConnect
    Anki(AnkiArgs),
}

#[derive(Args, Debug)]
pub struct AnkiArgs {
    /// AnkiConnect URL
    #[arg(long, default_value = "http:///localhost:8765")]
    pub url: String,

    /// Deck name(s) to import (repeatable; skips interactive deck picker)
    #[arg(long)]
    pub deck: Vec<String>,

    /// Field to use as the Japanese text
    #[arg(long)]
    pub japanese_field: Option<String>,

    /// Field to use as the English translation
    #[arg(long)]
    pub translation_field: Option<String>,

    /// Field containing Anki furigana markup (used if japanese field has none)
    #[arg(long)]
    pub furigana_field: Option<String>,

    /// Field to use as the romaji (romanized) reading
    #[arg(long)]
    pub romaji_field: Option<String>,

    /// Field to use as the source label
    #[arg(long)]
    pub source_field: Option<String>,

    /// Output directory (default: ~/.config/kotofetch/quotes/)
    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    /// Skip all prompts; use heuristic field mapping and overwrite existing files
    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum TranslationMode {
    None,
    English,
    Romaji,
    Furigana,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum FuriganaPosition {
    Above,
    Below,
}
