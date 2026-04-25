use crate::cli::AnkiArgs;
use dirs::config_dir;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// -- AnkiConnect client

struct AnkiClient {
    url: String,
}

impl AnkiClient {
    fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    fn invoke(&self, action: &str, params: Value) -> Result<Value> {
        let body = json!({
            "action": action,
            "version": 6,
            "params": params,
        });
        let response = ureq::post(&self.url)
            .send_json(body)
            .map_err(|e| format!("Could not reach AnkiConnect at {}: {e}", self.url))?;
        let mut resp_body = response.into_body();
        let json: Value = resp_body
            .read_json()
            .map_err(|e| format!("Failed to parse AnkiConnect response: {e}"))?;

        if let Some(err) = json["error"].as_str().filter(|s| !s.is_empty()) {
            return Err(format!("AnkiConnect error: {err}").into());
        }
        Ok(json["result"].clone())
    }

    fn deck_names(&self) -> Result<Vec<String>> {
        let result = self.invoke("deckNames", json!({}))?;
        let names = result
            .as_array()
            .ok_or("expected array of deck names")?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        Ok(names)
    }

    fn find_notes(&self, deck: &str) -> Result<Vec<u64>> {
        let result = self.invoke(
            "findNotes",
            json!({ "query": format!("deck:\"{}\"", deck) }),
        )?;
        let ids = result
            .as_array()
            .ok_or("expected array of note IDs")?
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();
        Ok(ids)
    }

    fn notes_info(&self, ids: &[u64]) -> Result<Vec<AnkiNote>> {
        let mut all = Vec::new();
        let chunks: Vec<&[u64]> = ids.chunks(500).collect();
        let multi = chunks.len() > 1;
        for (i, chunk) in chunks.iter().enumerate() {
            if multi {
                print!(
                    "\r  Fetching notes {}/{}...",
                    (i + 1) * chunk.len(),
                    ids.len()
                );
                io::stdout().flush().ok();
            }
            let result = self.invoke("notesInfo", json!({ "notes": chunk }))?;
            if let Some(arr) = result.as_array() {
                for val in arr {
                    if let Some(note) = parse_anki_note(val) {
                        all.push(note);
                    }
                }
            }
        }
        if multi {
            println!();
        }
        Ok(all)
    }
}

struct AnkiNote {
    fields: HashMap<String, String>,
    field_order: Vec<String>,
}

fn parse_anki_note(val: &Value) -> Option<AnkiNote> {
    let fields_obj = val["fields"].as_object()?;
    let mut fields = HashMap::new();
    let mut ordered: Vec<(String, usize)> = Vec::new();
    for (name, fv) in fields_obj {
        let value = fv["value"].as_str().unwrap_or("").to_string();
        let order = fv["order"].as_u64().unwrap_or(0) as usize;
        fields.insert(name.clone(), value);
        ordered.push((name.clone(), order));
    }
    ordered.sort_by_key(|(_, o)| *o);
    Some(AnkiNote {
        fields,
        field_order: ordered.into_iter().map(|(n, _)| n).collect(),
    })
}

// -- Field cleaning

fn remove_sound_markers(s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("[sound:") {
        result.push_str(&rest[..start]);
        match rest[start..].find(']') {
            Some(end) => rest = &rest[start + end + 1..],
            None => {
                result.push_str(&rest[start..]);
                return result;
            }
        }
    }
    result.push_str(rest);
    result
}

fn convert_html_ruby(s: &str) -> String {
    // <ruby>base<rt>reading</rt></ruby> or <ruby><rb>base</rb><rt>reading</rt></ruby>
    // -> base(reading)
    let mut result = String::new();
    let lower = s.to_lowercase();
    let mut pos = 0;

    while pos < s.len() {
        match lower[pos..].find("<ruby") {
            None => {
                result.push_str(&s[pos..]);
                break;
            }
            Some(rel) => {
                let abs = pos + rel;
                result.push_str(&s[pos..abs]);
                match s[abs..].find('>') {
                    None => {
                        result.push_str(&s[abs..]);
                        break;
                    }
                    Some(tag_end) => {
                        let inner_start = abs + tag_end + 1;
                        let lower_from = inner_start.min(lower.len());
                        match lower[lower_from..].find("</ruby>") {
                            None => {
                                pos = inner_start;
                            }
                            Some(ruby_end_rel) => {
                                let inner = &s[inner_start..inner_start + ruby_end_rel];
                                result.push_str(&extract_ruby_content(inner));
                                pos = inner_start + ruby_end_rel + "</ruby>".len();
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

fn extract_ruby_content(inner: &str) -> String {
    let lower = inner.to_lowercase();

    let base = if let (Some(s), Some(e)) = (lower.find("<rb>"), lower.find("</rb>")) {
        &inner[s + 4..e]
    } else {
        let rt_pos = lower.find("<rt>").unwrap_or(inner.len());
        &inner[..rt_pos]
    };

    let reading = if let (Some(s), Some(e)) = (lower.find("<rt>"), lower.find("</rt>")) {
        &inner[s + 4..e]
    } else {
        ""
    };

    if reading.is_empty() || base.is_empty() {
        inner.to_string()
    } else {
        format!("{}({})", base, reading)
    }
}

fn strip_tags_to_newlines(s: &str) -> String {
    // <br*>, <div*>, <p*> (opening) -> \n; everything else stripped
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' {
            let mut tag = String::new();
            i += 1;
            while i < chars.len() && chars[i] != '>' {
                tag.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip '>'
            }
            let tl = tag.trim_start_matches('/').trim().to_lowercase();
            let name = tl.split_whitespace().next().unwrap_or("");
            if matches!(name, "br" | "div" | "p") {
                result.push('\n');
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

pub fn clean_field(raw: &str) -> String {
    let s = remove_sound_markers(raw);
    let s = convert_html_ruby(&s);
    let s = strip_tags_to_newlines(&s);
    let s = decode_html_entities(&s);
    // Collapse runs of 3+ newlines to 2
    let mut s = s;
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim().to_string()
}

// -- Furigana conversion

fn is_kanji_char(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{20000}'..='\u{2A6DF}')
}

// Detect Anki's kanji[reading] syntax in a cleaned string.
fn has_anki_ruby(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len().saturating_sub(1) {
        if is_kanji_char(chars[i]) && chars[i + 1] == '[' {
            return true;
        }
    }
    false
}

// Detect our inline (reading) syntax.
fn has_inline_ruby(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len().saturating_sub(1) {
        if is_kanji_char(chars[i]) && chars[i + 1] == '(' {
            return true;
        }
    }
    false
}

fn has_ruby_syntax(s: &str) -> bool {
    has_anki_ruby(s) || has_inline_ruby(s)
}

// Convert Anki's `kanji[reading]` notation to our `kanji(reading)` notation.
// Also collapses the separator space Anki puts between groups when the next
// group starts with a kanji.
pub fn anki_furigana_to_inline(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if is_kanji_char(c) {
            // Accumulate kanji run
            let mut kanji = String::new();
            kanji.push(c);
            i += 1;
            while i < chars.len() && is_kanji_char(chars[i]) {
                kanji.push(chars[i]);
                i += 1;
            }
            // Check for [reading]
            if i < chars.len() && chars[i] == '[' {
                let mut reading = String::new();
                i += 1; // skip '['
                while i < chars.len() && chars[i] != ']' && chars[i] != '\n' {
                    reading.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() && chars[i] == ']' {
                    i += 1; // skip ']'
                }
                result.push_str(&kanji);
                result.push('(');
                result.push_str(&reading);
                result.push(')');
                // Collapse the separator space between ] and the next kanji
                if i < chars.len() && chars[i] == ' ' {
                    let next_cjk = chars[i + 1..]
                        .iter()
                        .find(|&&pc| pc != ' ')
                        .map(|&pc| is_kanji_char(pc))
                        .unwrap_or(false);
                    if next_cjk {
                        i += 1; // skip separator space
                    }
                }
            } else {
                result.push_str(&kanji);
            }
        } else {
            result.push(c);
            i += 1;
        }
    }
    result
}

// -- Field guessing

const JP_HINTS: &[&str] = &[
    "Expression",
    "Sentence",
    "Japanese",
    "Front",
    "Word",
    "Text",
];
const TL_HINTS: &[&str] = &["Meaning", "Translation", "English", "Definition", "Back"];
const FU_HINTS: &[&str] = &["Reading", "Furigana"];
const RO_HINTS: &[&str] = &["Romaji", "Romanization", "Romaji Reading"];
const SRC_HINTS: &[&str] = &["Source", "Notes", "Deck"];

fn guess_field(fields: &[String], hints: &[&str]) -> Option<String> {
    for hint in hints {
        if let Some(f) = fields
            .iter()
            .find(|f| f.to_lowercase() == hint.to_lowercase())
        {
            return Some(f.clone());
        }
    }
    None
}

// -- Interactive prompts

fn prompt(question: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{}: ", question);
    } else {
        print!("{} [{}]: ", question, default);
    }
    io::stdout().flush().unwrap();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).unwrap();
    let line = line.trim().to_string();
    if line.is_empty() && !default.is_empty() {
        default.to_string()
    } else {
        line
    }
}

fn prompt_field(
    label: &str,
    field_names: &[String],
    hint: Option<&str>,
    required: bool,
) -> Result<Option<String>> {
    let default = hint.unwrap_or("");
    let suffix = if required { "" } else { " (or skip)" };
    let input = prompt(&format!("Choose field for {}{}", label, suffix), default);

    if input.is_empty() {
        if required {
            return Err(format!("A field for '{label}' is required").into());
        }
        return Ok(None);
    }
    if !field_names.contains(&input) {
        return Err(format!("Unknown field: {input}").into());
    }
    Ok(Some(input))
}

// -- Field mapping

struct FieldMapping {
    japanese: String,
    translation: Option<String>,
    furigana: Option<String>,
    romaji: Option<String>,
    source: Option<String>,
    deck_name: String,
}

fn print_sample(samples: &[AnkiNote], idx: usize, deck: &str) {
    let sample = &samples[idx];
    let field_names = &sample.field_order;
    println!(
        "\nDeck \"{}\" - sample note ({}/{}):",
        deck,
        idx + 1,
        samples.len()
    );
    for name in field_names {
        let raw = sample.fields.get(name).map(|s| s.as_str()).unwrap_or("");
        let clean = clean_field(raw);
        let preview: String = clean.chars().take(40).collect();
        let preview = if clean.len() > preview.len() {
            format!("{}...", preview)
        } else {
            preview
        };
        println!("  {:<22} - {}", name, preview);
    }
    println!();
}

fn get_field_mapping(
    field_names: &[String],
    samples: &[AnkiNote],
    deck: &str,
    args: &AnkiArgs,
) -> Result<FieldMapping> {
    let jp_hint = args
        .japanese_field
        .clone()
        .or_else(|| guess_field(field_names, JP_HINTS));
    let tl_hint = args
        .translation_field
        .clone()
        .or_else(|| guess_field(field_names, TL_HINTS));
    let fu_hint = args
        .furigana_field
        .clone()
        .or_else(|| guess_field(field_names, FU_HINTS));
    let ro_hint = args
        .romaji_field
        .clone()
        .or_else(|| guess_field(field_names, RO_HINTS));
    let src_hint = args
        .source_field
        .clone()
        .or_else(|| guess_field(field_names, SRC_HINTS));

    if args.yes {
        let japanese =
            jp_hint.ok_or("Cannot determine japanese field automatically; use --japanese-field")?;
        return Ok(FieldMapping {
            japanese,
            translation: tl_hint,
            furigana: fu_hint,
            romaji: ro_hint,
            source: src_hint,
            deck_name: deck.to_string(),
        });
    }

    // Show sample note, let user cycle through samples if needed
    let mut idx = 0;
    loop {
        print_sample(samples, idx, deck);
        if samples.len() > 1 {
            let input = prompt(
                "Press Enter to map fields, or type 'next' for another sample",
                "",
            );
            if input.trim().eq_ignore_ascii_case("next") {
                idx = (idx + 1) % samples.len();
                continue;
            }
        }
        break;
    }

    let japanese = prompt_field("japanese", field_names, jp_hint.as_deref(), true)?.unwrap();
    let translation = prompt_field("translation", field_names, tl_hint.as_deref(), false)?;
    let furigana = prompt_field("furigana", field_names, fu_hint.as_deref(), false)?;
    let romaji = prompt_field("romaji", field_names, ro_hint.as_deref(), false)?;
    let source = prompt_field("source", field_names, src_hint.as_deref(), false)?;

    Ok(FieldMapping {
        japanese,
        translation,
        furigana,
        romaji,
        source,
        deck_name: deck.to_string(),
    })
}

// -- Note conversion

struct ImportedQuote {
    japanese: String,
    translation: Option<String>,
    romaji: Option<String>,
    source: Option<String>,
}

fn convert_note(note: &AnkiNote, mapping: &FieldMapping) -> Option<ImportedQuote> {
    let raw_jp = note.fields.get(&mapping.japanese)?;
    let mut japanese = clean_field(raw_jp);

    // If no ruby in the japanese field, check the dedicated furigana field
    if !has_ruby_syntax(&japanese)
        && let Some(fu_field) = &mapping.furigana
        && let Some(raw_fu) = note.fields.get(fu_field)
    {
        let fu = clean_field(raw_fu);
        if has_ruby_syntax(&fu) {
            japanese = fu;
        }
    }

    // Convert Anki's bracket notation to our inline parenthesis notation
    japanese = anki_furigana_to_inline(&japanese);

    if japanese.is_empty() {
        return None;
    }

    let translation = mapping
        .translation
        .as_ref()
        .and_then(|f| note.fields.get(f))
        .map(|v| clean_field(v))
        .filter(|s| !s.is_empty());

    let romaji = mapping
        .romaji
        .as_ref()
        .and_then(|f| note.fields.get(f))
        .map(|v| clean_field(v))
        .filter(|s| !s.is_empty());

    let source = mapping
        .source
        .as_ref()
        .and_then(|f| note.fields.get(f))
        .map(|v| clean_field(v))
        .filter(|s| !s.is_empty())
        .or_else(|| Some(mapping.deck_name.clone()));

    Some(ImportedQuote {
        japanese,
        translation,
        romaji,
        source,
    })
}

// -- TOML output

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
        .replace('\t', "\\t")
}

fn write_quotes_toml(path: &PathBuf, quotes: &[ImportedQuote]) -> std::io::Result<()> {
    let mut content = String::new();
    for q in quotes {
        content.push_str("[[quote]]\n");
        content.push_str(&format!("japanese = \"{}\"\n", toml_escape(&q.japanese)));
        if let Some(t) = &q.translation {
            content.push_str(&format!("translation = \"{}\"\n", toml_escape(t)));
        }
        if let Some(r) = &q.romaji {
            content.push_str(&format!("romaji = \"{}\"\n", toml_escape(r)));
        }
        if let Some(s) = &q.source {
            content.push_str(&format!("source = \"{}\"\n", toml_escape(s)));
        }
        content.push('\n');
    }
    fs::write(path, content)
}

// -- Deck selection

fn sanitize_deck_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            ' ' | ':' | '_' => '-',
            _ => '-',
        })
        .collect::<String>()
        // collapse multiple dashes
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn select_decks(decks: &[String], args: &AnkiArgs) -> Result<Vec<String>> {
    if !args.deck.is_empty() {
        for d in &args.deck {
            if !decks.contains(d) {
                return Err(
                    format!("Deck not found: \"{d}\"\nAvailable: {}", decks.join(", ")).into(),
                );
            }
        }
        return Ok(args.deck.clone());
    }

    let mut sorted = decks.to_vec();
    sorted.sort();
    println!("Found {} decks:", sorted.len());
    for (i, d) in sorted.iter().enumerate() {
        println!("  [{i}] {d}");
    }

    let input = prompt("Select decks (comma-separated indices, or 'all')", "0");

    if input.trim() == "all" {
        return Ok(sorted);
    }

    let mut selected = Vec::new();
    for part in input.split(',') {
        let idx: usize = part
            .trim()
            .parse()
            .map_err(|_| format!("Invalid index: \"{}\"", part.trim()))?;
        if idx >= sorted.len() {
            return Err(format!("Index {idx} is out of range (0–{})", sorted.len() - 1).into());
        }
        selected.push(sorted[idx].clone());
    }
    Ok(selected)
}

// -- Entry point

pub fn run_init(args: &AnkiArgs) -> Result<()> {
    println!("Connecting to AnkiConnect at {} ...", args.url);
    let client = AnkiClient::new(&args.url);

    let all_decks = client.deck_names()?;
    let selected = select_decks(&all_decks, args)?;

    let out_dir = if let Some(d) = &args.output_dir {
        d.clone()
    } else {
        let mut p = config_dir().ok_or("could not determine config directory")?;
        p.push("kotofetch/quotes");
        p
    };
    fs::create_dir_all(&out_dir)?;

    for deck in &selected {
        let note_ids = client.find_notes(deck)?;
        if note_ids.is_empty() {
            println!("Deck \"{deck}\" is empty, skipping.");
            continue;
        }

        // Fetch a small batch of notes to use as samples
        let sample_count = note_ids.len().min(10);
        let samples = client.notes_info(&note_ids[..sample_count])?;
        if samples.is_empty() {
            println!("Deck \"{deck}\" returned no notes, skipping.");
            continue;
        }
        let field_names = samples[0].field_order.clone();

        let mapping = get_field_mapping(&field_names, &samples, deck, args)?;

        // Fetch all notes
        let notes = client.notes_info(&note_ids)?;

        let mut quotes = Vec::new();
        let mut skipped = 0usize;
        for note in &notes {
            match convert_note(note, &mapping) {
                Some(q) => quotes.push(q),
                None => skipped += 1,
            }
        }

        let filename = sanitize_deck_name(deck) + ".toml";
        let out_path = out_dir.join(&filename);

        if out_path.exists() && !args.yes {
            let answer = prompt(
                &format!("{} already exists. Overwrite? (y/n)", out_path.display()),
                "y",
            );
            if !answer.starts_with('y') && !answer.starts_with('Y') {
                println!("Skipping \"{deck}\".");
                continue;
            }
        }

        println!(
            "Importing {} notes from \"{}\" -> {}",
            quotes.len(),
            deck,
            out_path.display()
        );
        if skipped > 0 {
            println!("  (skipped {skipped} notes with empty japanese field)");
        }
        write_quotes_toml(&out_path, &quotes)?;
    }

    if selected.len() == 1 {
        let mode = sanitize_deck_name(&selected[0]);
        println!("\nDone. Run: kotofetch --modes {mode}");
    } else {
        println!("\nDone.");
    }

    Ok(())
}
