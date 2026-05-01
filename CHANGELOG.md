# Changelog

## v0.2.20

### Added
- Multiple translations display
- New furigana position option
- New quotes

### Fixed
- Quotes translations issues 


## v0.2.19

### Added
- Anki integration: import decks as kotofetch quote files via `kotofetch init anki` (requires Anki running with AnkiConnect plugin)
- Anki importer supports interactive deck selection, field mapping, and furigana conversion from Anki's `kanji[reading]` syntax
- Sample navigation during Anki import: type `next` to cycle through up to 10 sample notes when the first card is unhelpful
- Non-interactive Anki import mode via flags (`--deck`, `--japanese-field`, `--translation-field`, etc.) for scripting
- Furigana display mode (`--translation furigana`): readings are shown centered below their kanji using inline `kanji(reading)` markup
- Support for compound-word furigana annotations (e.g. `大事(だいじ)`)
- New built-in quotes: *A Place Further Than the Universe*, *My Dress-Up Darling*, *Toradora!*
- New Japanese proverbs about growth, resilience, and everyday wisdom

### Fixed
- NixOS install failure
