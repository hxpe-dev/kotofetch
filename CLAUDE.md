# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build               # debug build
cargo build --release     # release build
cargo run                 # run with default config
cargo run -- --modes anime --translation romaji  # pass CLI args
cargo clippy              # lint
cargo fmt                 # format
```

There are no automated tests in this project.

## Architecture

`kotofetch` is a Rust CLI tool (~5 source files) that displays a random Japanese quote in a styled terminal box.

**Data flow:** `main.rs` → parse CLI (`cli.rs`) → load + merge config (`config.rs`) → render (`display.rs`), pulling quotes from (`quotes.rs`).

### Config layering (`config.rs`)

Three layers merged in order (last wins):
1. `RuntimeConfig::default()` — hardcoded defaults
2. `~/.config/kotofetch/config.toml` — user file config (`FileConfig` / `DisplayConfig`)
3. CLI flags — `Cli` struct fields (all `Option<T>`, `None` means "not overridden")

`make_runtime_config()` applies layers 2 and 3 onto a default `RuntimeConfig`.

### Quote loading (`display.rs:render`, `quotes.rs`)

Built-in quote files (`quotes/*.toml`) are compiled into the binary via `include_str!` in `BUILTIN_QUOTES`. At runtime, `render()` iterates `runtime.modes`, for each mode:
1. Checks `~/.config/kotofetch/quotes/<name>.toml` — if present, uses that file (user file takes priority, built-in is skipped).
2. Falls back to the matching `BUILTIN_QUOTES` entry.

Quote TOML structure:
```toml
[[quote]]
japanese = "..."
translation = "..."   # optional
romaji = "..."        # optional
source = "..."        # optional
furigana = "..."      # optional; space-separated readings, one per kanji in order
```

### Display pipeline (`display.rs`)

- `simulate_font_size()` — adds spaces between characters for medium/large sizes.
- `format_furigana()` — aligns kana readings under each kanji character using Unicode widths.
- `color_from_hex()` — resolves named ANSI colors or `#RRGGBB` hex → nearest ANSI 256 index.
- `print_boxed()` — computes inner width, draws border + padding, renders Japanese, furigana, translation, and source blocks.
- `render()` — static mode prints once; `dynamic = true` mode loops with 200 ms polling, redraws on terminal resize, exits on Ctrl+C.

## Release process

See `DEV_NOTE.md` for the full release checklist (version bump in `Cargo.toml`, git tag, AUR `PKGBUILD` update, `default.nix` sha256 bump).
