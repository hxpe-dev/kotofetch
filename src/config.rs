use dirs::config_dir;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct FileConfig {
    pub display: Option<DisplayConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TranslationMode {
    None,
    English,
    Romaji,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DisplayConfig {
    pub horizontal_padding: Option<usize>,
    pub vertical_padding: Option<usize>,
    pub width: Option<usize>,
    pub show_translation: Option<TranslationMode>,
    pub translation_color: Option<String>,
    pub quote_color: Option<String>,
    pub font_size: Option<String>,
    pub bold: Option<bool>,
    pub border: Option<bool>,
    pub rounded_border: Option<bool>,
    pub border_color: Option<String>,
    pub source: Option<bool>,
    pub modes: Option<Vec<PathBuf>>,
    pub seed: Option<u64>,
    pub centered: Option<bool>,
    pub dynamic: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub horizontal_padding: usize,
    pub vertical_padding: usize,
    pub width: usize,
    pub show_translation: TranslationMode,
    pub translation_color: String,
    pub quote_color: String,
    pub font_size: String,
    pub bold: bool,
    pub border: bool,
    pub rounded_border: bool,
    pub border_color: String,
    pub source: bool,
    pub modes: Vec<PathBuf>,
    pub seed: u64,
    pub centered: bool,
    pub dynamic: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            horizontal_padding: 3,
            vertical_padding: 1,
            width: 0, // 0 = automatic
            show_translation: TranslationMode::English,
            translation_color: "dim".to_string(),
            quote_color: "white".to_string(),
            border_color: "white".to_string(),
            font_size: "medium".to_string(),
            bold: true,
            border: true,
            rounded_border: true,
            source: false,
            // by default look for example TOML files
            modes: vec![
                PathBuf::from("proverb.toml"),
                PathBuf::from("haiku.toml"),
                PathBuf::from("anime.toml"),
                PathBuf::from("lyrics.toml"),
                PathBuf::from("yojijukugo.toml"),
            ],
            seed: 0, // 0 = random
            centered: true,
            dynamic: false,
        }
    }
}

pub fn load_user_config(path_override: Option<PathBuf>) -> Option<FileConfig> {
    let path = if let Some(p) = path_override {
        p
    } else if let Some(mut d) = config_dir() {
        d.push("kotofetch/config.toml");
        d
    } else {
        return None;
    };

    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(s) => match toml::from_str::<FileConfig>(&s) {
                Ok(parsed) => Some(parsed),
                Err(e) => {
                    eprintln!("Failed to parse config.toml: {}", e);
                    None
                }
            },
            Err(e) => {
                eprintln!("Failed to read config: {}", e);
                None
            }
        }
    } else {
        None
    }
}

pub fn make_runtime_config(user: Option<FileConfig>, cli: &crate::cli::Cli) -> RuntimeConfig {
    let mut r = RuntimeConfig::default();

    // apply user file config
    if let Some(uf) = user {
        if let Some(d) = uf.display {
            if let Some(p) = d.horizontal_padding {
                r.horizontal_padding = p;
            }
            if let Some(p) = d.vertical_padding {
                r.vertical_padding = p;
            }
            if let Some(w) = d.width {
                r.width = w;
            }
            if let Some(st) = d.show_translation {
                r.show_translation = st;
            }
            if let Some(tc) = d.translation_color {
                r.translation_color = tc;
            }
            if let Some(qc) = d.quote_color {
                r.quote_color = qc;
            }
            if let Some(fs) = d.font_size {
                r.font_size = fs;
            }
            if let Some(b) = d.bold {
                r.bold = b;
            }
            if let Some(b) = d.border {
                r.border = b;
            }
            if let Some(b) = d.rounded_border {
                r.rounded_border = b;
            }
            if let Some(bc) = d.border_color {
                r.border_color = bc;
            }
            if let Some(b) = d.source {
                r.source = b;
            }
            if let Some(m) = d.modes {
                r.modes = m.into_iter().map(|s| PathBuf::from(s)).collect();
            }
            if let Some(s) = d.seed {
                r.seed = s;
            }
            if let Some(c) = d.centered {
                r.centered = c;
            }
            if let Some(dc) = d.dynamic {
                r.dynamic = dc;
            }
        }
    }

    // apply CLI overrides
    if let Some(p) = cli.horizontal_padding {
        r.horizontal_padding = p;
    }
    if let Some(p) = cli.vertical_padding {
        r.vertical_padding = p;
    }
    if let Some(w) = cli.width {
        r.width = w;
    }

    // map CLI TranslationMode -> config::TranslationMode
    if let Some(tmode) = &cli.translation {
        r.show_translation = match tmode {
            crate::cli::TranslationMode::None => TranslationMode::None,
            crate::cli::TranslationMode::English => TranslationMode::English,
            crate::cli::TranslationMode::Romaji => TranslationMode::Romaji,
        };
    }

    if let Some(tc) = &cli.translation_color {
        r.translation_color = tc.clone();
    }
    if let Some(qc) = &cli.quote_color {
        r.quote_color = qc.clone();
    }
    if let Some(b) = cli.bold {
        r.bold = b;
    }
    if let Some(b) = cli.border {
        r.border = b;
    }
    if let Some(b) = cli.rounded_border {
        r.rounded_border = b;
    }
    if let Some(bc) = &cli.border_color {
        r.border_color = bc.clone();
    }
    if let Some(b) = cli.source {
        r.source = b;
    }

    if let Some(cli_modes) = &cli.modes {
        r.modes = cli_modes.clone();
    }

    if let Some(s) = cli.seed {
        r.seed = s;
    }
    if let Some(c) = cli.centered {
        r.centered = c;
    }
    if let Some(dc) = cli.dynamic {
        r.dynamic = dc;
    }

    r
}
