//! Multilingual spellcheck & probabilistic smartphone-style word prediction.
//!
//! Supports French, English, German, Spanish and Italian, detected automatically from the
//! operating system keyboard layout or locale (and switchable per exercise). Combines frequent
//! everyday/school vocabularies with OS Hunspell / wordlist dictionaries when available on Linux,
//! Windows and macOS, using unigram/bigram probability ranking, SymSpell 1-edit lookup,
//! Jaro-Winkler similarity and keyboard-proximity typo repair.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Maximum number of suggestions shown in the word prediction/correction popup.
pub const MAX_SUGGESTIONS: usize = 3;

/// Supported spellcheck & prediction languages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpellLang {
    #[default]
    Fr,
    En,
    De,
    Es,
    It,
}

impl SpellLang {
    pub const ALL: [SpellLang; 5] = [SpellLang::Fr, SpellLang::En, SpellLang::De, SpellLang::Es, SpellLang::It];

    pub fn code(self) -> &'static str {
        match self {
            SpellLang::Fr => "fr",
            SpellLang::En => "en",
            SpellLang::De => "de",
            SpellLang::Es => "es",
            SpellLang::It => "it",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            SpellLang::Fr => "FR",
            SpellLang::En => "EN",
            SpellLang::De => "DE",
            SpellLang::Es => "ES",
            SpellLang::It => "IT",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SpellLang::Fr => "Français",
            SpellLang::En => "English",
            SpellLang::De => "Deutsch",
            SpellLang::Es => "Español",
            SpellLang::It => "Italiano",
        }
    }

    pub fn tts_locale(self) -> &'static str {
        match self {
            SpellLang::Fr => "fr-FR",
            SpellLang::En => "en-US",
            SpellLang::De => "de-DE",
            SpellLang::Es => "es-ES",
            SpellLang::It => "it-IT",
        }
    }

    pub fn edge_voice(self) -> &'static str {
        match self {
            SpellLang::Fr => "fr-FR-DeniseNeural",
            SpellLang::En => "en-US-JennyNeural",
            SpellLang::De => "de-DE-KatjaNeural",
            SpellLang::Es => "es-ES-ElviraNeural",
            SpellLang::It => "it-IT-ElsaNeural",
        }
    }

    pub fn piper_model_name(self) -> &'static str {
        match self {
            SpellLang::Fr => "fr_FR-siwis-medium.onnx",
            SpellLang::En => "en_US-lessac-medium.onnx",
            SpellLang::De => "de_DE-thorsten-medium.onnx",
            SpellLang::Es => "es_ES-davefx-medium.onnx",
            SpellLang::It => "it_IT-paola-medium.onnx",
        }
    }

    pub fn espeak_voice(self) -> &'static str {
        match self {
            SpellLang::Fr => "fr",
            SpellLang::En => "en",
            SpellLang::De => "de",
            SpellLang::Es => "es",
            SpellLang::It => "it",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SpellLang::Fr => SpellLang::En,
            SpellLang::En => SpellLang::De,
            SpellLang::De => SpellLang::Es,
            SpellLang::Es => SpellLang::It,
            SpellLang::It => SpellLang::Fr,
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        let lower = tag.trim().to_ascii_lowercase();
        if lower.is_empty() || lower == "c" || lower == "posix" {
            return None;
        }
        let primary = lower.split(['-', '_', '.', '@']).next().unwrap_or("");
        match primary {
            "fr" | "french" | "fra" => Some(SpellLang::Fr),
            "en" | "english" | "eng" => Some(SpellLang::En),
            "de" | "german" | "deu" | "ger" => Some(SpellLang::De),
            "es" | "spanish" | "spa" => Some(SpellLang::Es),
            "it" | "italian" | "ita" => Some(SpellLang::It),
            _ => None,
        }
    }

    /// Detects the active OS keyboard / locale language among FR / EN / DE / ES / IT.
    pub fn detect_from_os(fallback_ui_lang: &str) -> Self {
        if let Some(code) = crate::input_language::active_lang_code()
            && let Some(lang) = Self::from_tag(code)
        {
            return lang;
        }
        for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(val) = std::env::var(key) {
                for part in val.split(':') {
                    if let Some(lang) = Self::from_tag(part) {
                        return lang;
                    }
                }
            }
        }
        Self::from_tag(fallback_ui_lang).unwrap_or(SpellLang::Fr)
    }
}

/// Analysis result for the word currently under the caret, used by the floating prediction popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordAnalysis {
    pub word: String,
    pub char_range: (usize, usize),
    pub is_misspelled: bool,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Debug)]
struct LexEntry {
    word: String,
    lower: String,
    ascii: String,
    freq_rank: u32,
}

struct Lexicon {
    exact_lower: HashSet<String>,
    ascii_map: HashMap<String, Vec<usize>>,
    prefix1_map: HashMap<char, Vec<usize>>,
    prefix2_map: HashMap<(char, char), Vec<usize>>,
    delete_map: HashMap<String, Vec<usize>>,
    entries: Vec<LexEntry>,
    hunspell: Option<spellbook::Dictionary>,
}

static LEX_FR: OnceLock<Lexicon> = OnceLock::new();
static LEX_EN: OnceLock<Lexicon> = OnceLock::new();
static LEX_DE: OnceLock<Lexicon> = OnceLock::new();
static LEX_ES: OnceLock<Lexicon> = OnceLock::new();
static LEX_IT: OnceLock<Lexicon> = OnceLock::new();

fn lexicon(lang: SpellLang) -> &'static Lexicon {
    match lang {
        SpellLang::Fr => LEX_FR.get_or_init(|| build_lexicon(SpellLang::Fr)),
        SpellLang::En => LEX_EN.get_or_init(|| build_lexicon(SpellLang::En)),
        SpellLang::De => LEX_DE.get_or_init(|| build_lexicon(SpellLang::De)),
        SpellLang::Es => LEX_ES.get_or_init(|| build_lexicon(SpellLang::Es)),
        SpellLang::It => LEX_IT.get_or_init(|| build_lexicon(SpellLang::It)),
    }
}

fn build_lexicon(lang: SpellLang) -> Lexicon {
    let mut lex = Lexicon {
        exact_lower: HashSet::new(),
        ascii_map: HashMap::new(),
        prefix1_map: HashMap::new(),
        prefix2_map: HashMap::new(),
        delete_map: HashMap::new(),
        entries: Vec::new(),
        hunspell: None,
    };
    for (idx, word) in builtin_words(lang).iter().enumerate() {
        push_word(&mut lex, word, u32::try_from(idx).unwrap_or(u32::MAX));
    }
    let os_base_rank = 10_000u32;
    let mut os_added = 0u32;
    let max_os_words = 250_000u32;
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    for path in os_dictionary_paths(lang) {
        if os_added >= max_os_words {
            break;
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !seen_paths.insert(canonical) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            let text = decode_dict_bytes(&bytes);
            let is_hunspell = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("dic"));
            if is_hunspell && lex.hunspell.is_none() {
                let aff_path = path.with_extension("aff");
                if aff_path.is_file()
                    && let Ok(aff_bytes) = std::fs::read(&aff_path)
                {
                    let aff_text = decode_dict_bytes(&aff_bytes);
                    lex.hunspell = spellbook::Dictionary::new(&aff_text, &text).ok();
                }
            }
            for (line_idx, raw) in text.lines().enumerate() {
                if is_hunspell && line_idx == 0 && raw.trim().chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let mut parts = raw.split(['/', '\t', ' ']);
                let cleaned = parts.next().unwrap_or("").trim();
                let flags = parts.next().unwrap_or("");
                if cleaned.chars().count() < 2 || cleaned.chars().count() > 32 || !cleaned.chars().all(|c| c.is_alphabetic() || c == '-') {
                    continue;
                }
                if cleaned.ends_with('-') || cleaned.starts_with('-') {
                    continue;
                }
                push_word(&mut lex, cleaned, os_base_rank.saturating_add(os_added));
                os_added = os_added.saturating_add(1);
                if is_hunspell && !flags.is_empty() && !cleaned.contains('-') {
                    if flags.contains('S') || flags.contains('s') {
                        let plural = format!("{cleaned}s");
                        push_word(&mut lex, &plural, os_base_rank.saturating_add(os_added));
                        os_added = os_added.saturating_add(1);
                    }
                    if flags.contains('X') {
                        let plural_x = format!("{cleaned}x");
                        push_word(&mut lex, &plural_x, os_base_rank.saturating_add(os_added));
                        os_added = os_added.saturating_add(1);
                    }
                    if flags.contains('F') && lang == SpellLang::Fr {
                        for form in [format!("{cleaned}e"), format!("{cleaned}s"), format!("{cleaned}es")] {
                            push_word(&mut lex, &form, os_base_rank.saturating_add(os_added));
                            os_added = os_added.saturating_add(1);
                        }
                    }
                }
                if os_added >= max_os_words {
                    break;
                }
            }
        }
    }
    lex
}

fn push_word(lex: &mut Lexicon, word: &str, freq_rank: u32) {
    let trimmed = word.trim();
    if trimmed.is_empty() {
        return;
    }
    let lower = trimmed.to_lowercase();
    if !lex.exact_lower.insert(lower.clone()) {
        return;
    }
    let ascii = strip_accents(&lower);
    let id = lex.entries.len();
    lex.ascii_map.entry(ascii.clone()).or_default().push(id);
    let mut a_chars = ascii.chars();
    let first_ch = a_chars.next();
    let second_ch = a_chars.next();
    if freq_rank < 10_000
        && let Some(c0) = first_ch
    {
        lex.prefix1_map.entry(c0).or_default().push(id);
    }
    if let (Some(c0), Some(c1)) = (first_ch, second_ch) {
        lex.prefix2_map.entry((c0, c1)).or_default().push(id);
    }
    if freq_rank < 10_000 {
        let chars: Vec<char> = ascii.chars().collect();
        if (3..=16).contains(&chars.len()) {
            for skip in 0..chars.len() {
                let del: String = chars.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, &c)| c).collect();
                lex.delete_map.entry(del).or_default().push(id);
            }
        }
    }
    lex.entries.push(LexEntry { word: trimmed.to_owned(), lower, ascii, freq_rank });
}

fn decode_dict_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) && bytes.len() >= 2 {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().copied().map(u16::from_le_bytes).collect();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) && bytes.len() >= 2 {
        let units: Vec<u16> = bytes[2..].as_chunks::<2>().0.iter().copied().map(u16::from_be_bytes).collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn os_dictionary_paths(lang: SpellLang) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let names: &[&str] = match lang {
        SpellLang::Fr => &[
            "/usr/share/hunspell/fr_FR.dic",
            "/usr/share/hunspell/fr.dic",
            "/usr/share/myspell/fr_FR.dic",
            "/usr/share/dict/french",
            "/Library/Spelling/fr_FR.dic",
        ],
        SpellLang::En => &[
            "/usr/share/hunspell/en_US.dic",
            "/usr/share/hunspell/en_GB.dic",
            "/usr/share/dict/american-english",
            "/usr/share/dict/words",
            "/Library/Spelling/en_US.dic",
        ],
        SpellLang::De => &[
            "/usr/share/hunspell/de_DE.dic",
            "/usr/share/hunspell/de_DE_frami.dic",
            "/usr/share/dict/ngerman",
            "/usr/share/dict/german",
            "/Library/Spelling/de_DE.dic",
        ],
        SpellLang::Es => {
            &["/usr/share/hunspell/es_ES.dic", "/usr/share/hunspell/es_ANY.dic", "/usr/share/dict/spanish", "/Library/Spelling/es_ES.dic"]
        }
        SpellLang::It => &["/usr/share/hunspell/it_IT.dic", "/usr/share/dict/italian", "/Library/Spelling/it_IT.dic"],
    };
    for p in names {
        let path = PathBuf::from(p);
        if path.is_file() {
            out.push(path);
        }
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let sub = PathBuf::from(appdata).join("Microsoft").join("Spelling").join(lang.tts_locale());
        for fname in ["default.dic", "Added.dic"] {
            let p = sub.join(fname);
            if p.is_file() {
                out.push(p);
            }
        }
    }
    out
}

/// Strips Latin diacritics and normalizes German sharp-s to ASCII lowercase equivalents.
pub fn strip_accents(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' | 'Å' => out.push('a'),
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => out.push('e'),
            'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => out.push('i'),
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => out.push('o'),
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => out.push('u'),
            'ç' | 'Ç' => out.push('c'),
            'ñ' | 'Ñ' => out.push('n'),
            'ÿ' | 'ý' | 'Ÿ' | 'Ý' => out.push('y'),
            'œ' | 'Œ' => out.push_str("oe"),
            'æ' | 'Æ' => out.push_str("ae"),
            'ß' => out.push_str("ss"),
            other => {
                for lc in other.to_lowercase() {
                    out.push(lc);
                }
            }
        }
    }
    out
}

fn is_known_word(lex: &Lexicon, lang: SpellLang, lower: &str) -> bool {
    if lex.exact_lower.contains(lower) {
        return true;
    }
    if let Some(dict) = &lex.hunspell
        && (dict.check(lower) || dict.check(&capitalize_first(lower)))
    {
        return true;
    }
    // Check common plural / feminine / verb inflections if base form (>= 3 chars) exists.
    let chars = lower.chars().count();
    if chars >= 4 {
        let suffixes: &[&str] = match lang {
            SpellLang::Fr => &["s", "es", "x", "nt", "ait", "aient", "ons", "ez"],
            SpellLang::En => &["s", "es", "ed", "ing", "ly"],
            SpellLang::De => &["s", "es", "en", "er", "e", "n"],
            SpellLang::Es => &["s", "es", "as", "os"],
            SpellLang::It => &["i", "e", "a", "o"],
        };
        for suf in suffixes {
            if let Some(stem) = lower.strip_suffix(suf)
                && stem.chars().count() >= 3
                && (!matches!(*suf, "s" | "es" | "x") || !stem.ends_with(['s', 'x', 'z']))
                && lex.exact_lower.contains(stem)
            {
                return true;
            }
        }
        if lang == SpellLang::Fr {
            for suf in ["e", "es", "ons", "eons", "ez", "ent", "ais", "ait", "aient", "é", "ée", "és", "ées"] {
                if let Some(stem) = lower.strip_suffix(suf)
                    && stem.chars().count() >= 3
                    && lex.exact_lower.contains(&format!("{stem}er"))
                {
                    return true;
                }
            }
            if let Some(stem) = lower.strip_suffix("çons")
                && stem.chars().count() >= 2
                && lex.exact_lower.contains(&format!("{stem}cer"))
            {
                return true;
            }
        }
    }
    false
}

fn word_chars_with_ranges(text: &str) -> Vec<(usize, usize, String)> {
    // Build a mask of character indices belonging to inline markup (`**`, `<u>`, `<span>`, URLs)
    // using `pagetext::inline_pieces` so CSS/tags/URLs inside editable lines are never spellchecked.
    let chars: Vec<char> = text.chars().collect();
    let mut skip = vec![false; chars.len()];
    for (range, style) in crate::pagetext::inline_pieces(text) {
        if style == crate::pagetext::Style::Marker || style == crate::pagetext::Style::Link {
            let cs = text.get(..range.start).map_or(0, |s| s.chars().count());
            let ce = text.get(..range.end).map_or(chars.len(), |s| s.chars().count());
            if style == crate::pagetext::Style::Marker {
                for slot in skip.get_mut(cs..ce).into_iter().flatten() {
                    *slot = true;
                }
            }
        }
    }
    // Also skip `<...>` HTML tag spans literally present in edit buffer if any.
    let mut in_angle = false;
    for (idx, &ch) in chars.iter().enumerate() {
        if ch == '<' {
            in_angle = true;
        }
        if in_angle && let Some(slot) = skip.get_mut(idx) {
            *slot = true;
        }
        if ch == '>' {
            in_angle = false;
        }
    }

    let mut raw_tokens = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let is_word = !skip[i] && (chars[i].is_alphabetic() || chars[i].is_ascii_digit());
        if !is_word {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && !skip[i] && (chars[i].is_alphabetic() || chars[i].is_ascii_digit()) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        // Skip tokens with digits (numbers, formulas, dates) or URL domains.
        if word.chars().all(char::is_alphabetic) && !looks_like_url_tail(&chars, start, i) {
            raw_tokens.push((start, i, word));
        }
    }
    raw_tokens
}

fn looks_like_url_tail(chars: &[char], start: usize, end: usize) -> bool {
    let before: String = chars[start.saturating_sub(8)..start].iter().collect();
    if before.contains("://") || before.ends_with("www.") {
        return true;
    }
    chars.get(end..end.saturating_add(4)).is_some_and(|s| {
        let tail: String = s.iter().collect();
        tail.starts_with(".com") || tail.starts_with(".fr") || tail.starts_with(".org") || tail.starts_with(".net") || tail.starts_with(".pdf")
    })
}

/// Lightweight context model built on the fly from the open page and preceding sentence tokens
/// (trigrams, bigrams, and topic word frequencies) so predictions adapt to the subject and
/// preceding words like a mobile keyboard without bloated offline downloads.
struct PageModel {
    bigrams: HashMap<String, HashMap<String, u32>>,
    trigrams: HashMap<(String, String), HashMap<String, u32>>,
    topic_counts: HashMap<String, u32>,
    words: Vec<String>,
}

impl PageModel {
    fn from_sources(lex: &Lexicon, lang: SpellLang, other_blocks_context: &str, before_caret_text: &str) -> Self {
        let mut model = Self { bigrams: HashMap::new(), trigrams: HashMap::new(), topic_counts: HashMap::new(), words: Vec::new() };
        let mut seen_topic = HashSet::new();
        for src in [other_blocks_context, before_caret_text] {
            if src.trim().is_empty() {
                continue;
            }
            let chars: Vec<char> = src.chars().collect();
            let tokens = word_chars_with_ranges(src);
            for (i, (_, _, word)) in tokens.iter().enumerate() {
                let lower = word.to_lowercase();
                if lower.chars().count() < 2 || !is_known_word(lex, lang, &lower) {
                    continue;
                }
                let ascii = strip_accents(&lower);
                if lower.chars().count() >= 4 {
                    *model.topic_counts.entry(lower.clone()).or_insert(0) += 1;
                    if seen_topic.insert(lower.clone()) {
                        model.words.push(lower.clone());
                    }
                }
                if let Some(prev) = same_clause_prev(&chars, &tokens, i) {
                    let prev_a = strip_accents(&prev.to_lowercase());
                    *model.bigrams.entry(prev_a.clone()).or_default().entry(lower.clone()).or_insert(0) += 1;
                    if i >= 2
                        && let Some(prev2) = same_clause_prev(&chars, &tokens, i - 1)
                    {
                        let prev2_a = strip_accents(&prev2.to_lowercase());
                        *model.trigrams.entry((prev2_a, prev_a)).or_default().entry(lower.clone()).or_insert(0) += 1;
                    }
                }
                let _ = ascii;
            }
        }
        model
    }
}

struct WordCtx<'a> {
    prev2: Option<&'a str>,
    prev: Option<&'a str>,
    after_apostrophe: bool,
    page: &'a PageModel,
}

fn is_clause_gap(chars: &[char], from: usize, to: usize) -> bool {
    if from > to || to > chars.len() {
        return false;
    }
    chars[from..to].iter().all(|c| c.is_whitespace() || matches!(c, ',' | '\'' | '’' | '-'))
}

fn same_clause_prev<'a>(chars: &[char], tokens: &'a [(usize, usize, String)], idx: usize) -> Option<&'a str> {
    let prev_idx = idx.checked_sub(1)?;
    let prev_end = tokens[prev_idx].1;
    let cur_start = tokens[idx].0;
    is_clause_gap(chars, prev_end, cur_start).then_some(tokens[prev_idx].2.as_str())
}

fn preceding_context<'a>(chars: &[char], tokens: &'a [(usize, usize, String)], prev_idx: usize) -> (Option<&'a str>, Option<&'a str>) {
    let prev = Some(tokens[prev_idx].2.as_str());
    let prev2 = same_clause_prev(chars, tokens, prev_idx);
    (prev2, prev)
}

/// Analyses the word at `caret_char` using full page context (`page_context` from other blocks on
/// the open page plus earlier text before the caret). Supports 1-letter prefix prediction and
/// next-word prediction right after a space or apostrophe.
pub fn analyze_word_at_caret_with_context(text: &str, caret_char: usize, lang: SpellLang, page_context: &str) -> Option<WordAnalysis> {
    let chars: Vec<char> = text.chars().collect();
    let clamped = caret_char.min(chars.len());
    let tokens = word_chars_with_ranges(text);
    let lex = lexicon(lang);

    if let Some(pos) = tokens.iter().position(|(s, e, _)| clamped >= *s && clamped <= *e) {
        let (start, end, word) = tokens[pos].clone();
        let before_caret: String = chars[..start].iter().collect();
        let page = PageModel::from_sources(lex, lang, page_context, &before_caret);
        let prev = pos.checked_sub(1).and_then(|pi| {
            let p_end = tokens[pi].1;
            is_clause_gap(&chars, p_end, start).then_some(pi)
        });
        let (prev2_word, prev_word) = match prev {
            Some(pi) => preceding_context(&chars, &tokens, pi),
            None => (None, None),
        };
        let after_apostrophe = start > 0 && matches!(chars.get(start - 1), Some('\'' | '’'));
        let ctx = WordCtx { prev2: prev2_word, prev: prev_word, after_apostrophe, page: &page };
        let lower = word.to_lowercase();
        let is_misspelled = word.chars().count() >= 2 && !is_known_word(lex, lang, &lower);
        let suggestions = suggest_words(lex, lang, &word, is_misspelled, &ctx, MAX_SUGGESTIONS);
        if !is_misspelled && suggestions.is_empty() {
            return None;
        }
        return Some(WordAnalysis { word, char_range: (start, end), is_misspelled, suggestions });
    }

    // Caret is outside a word token: predict the next word right after space or apostrophe.
    if clamped == 0 {
        return None;
    }
    let prev_ch = chars.get(clamped - 1).copied()?;
    if !prev_ch.is_whitespace() && !matches!(prev_ch, '\'' | '’') {
        return None;
    }
    let prev_idx = tokens.iter().rposition(|(_, e, _)| *e < clamped)?;
    let prev_end = tokens[prev_idx].1;
    if !chars[prev_end..clamped].iter().all(|c| c.is_whitespace() || matches!(c, '\'' | '’')) {
        return None;
    }
    let after_apostrophe = matches!(prev_ch, '\'' | '’');
    let before_caret: String = chars[..clamped].iter().collect();
    let page = PageModel::from_sources(lex, lang, page_context, &before_caret);
    let (prev2_word, prev_word) = preceding_context(&chars, &tokens, prev_idx);
    let ctx = WordCtx { prev2: prev2_word, prev: prev_word, after_apostrophe, page: &page };
    let suggestions = next_word_predictions(lex, lang, &ctx, MAX_SUGGESTIONS);
    if suggestions.is_empty() {
        return None;
    }
    Some(WordAnalysis { word: String::new(), char_range: (clamped, clamped), is_misspelled: false, suggestions })
}

/// Analyses the word currently touching `caret_char` and returns smartphone-style probabilistic
/// suggestions (up to 3 words: trigram/bigram next-word or prefix prediction, accent fix, typo repair).
pub fn analyze_word_at_caret(text: &str, caret_char: usize, lang: SpellLang) -> Option<WordAnalysis> {
    analyze_word_at_caret_with_context(text, caret_char, lang, "")
}

/// Returns character ranges `(start, end)` of completed misspelled words (excluding the word
/// currently under `caret_char`) for subtle red underline feedback.
pub fn check_completed_words(text: &str, caret_char: Option<usize>, lang: SpellLang) -> Vec<(usize, usize)> {
    let lex = lexicon(lang);
    let mut bad = Vec::new();
    for (s, e, word) in word_chars_with_ranges(text) {
        if word.chars().count() < 2 {
            continue;
        }
        if caret_char.is_some_and(|at| at >= s && at <= e) {
            continue;
        }
        let lower = word.to_lowercase();
        if !is_known_word(lex, lang, &lower) {
            bad.push((s, e));
        }
    }
    bad
}

fn format_ranked_candidates(lex: &Lexicon, candidates: Vec<(i32, u32, String)>, capitalize: bool, limit: usize) -> Vec<String> {
    let mut ranked = candidates;
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2)));
    let mut out = Vec::with_capacity(limit);
    let mut seen = HashSet::new();
    for (_, _, raw) in ranked {
        let base = if let Some(ids) = lex.ascii_map.get(&strip_accents(&raw.to_lowercase()))
            && let Some(exact) = ids.iter().find(|&&id| lex.entries[id].lower == raw.to_lowercase())
        {
            lex.entries[*exact].word.clone()
        } else {
            raw
        };
        let s = if capitalize && !base.chars().next().is_some_and(char::is_uppercase) { capitalize_first(&base) } else { base };
        if seen.insert(s.to_lowercase()) {
            out.push(s);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

fn next_word_predictions(lex: &Lexicon, lang: SpellLang, ctx: &WordCtx<'_>, limit: usize) -> Vec<String> {
    let Some(prev) = ctx.prev else {
        return Vec::new();
    };
    let p_ascii = strip_accents(&prev.to_lowercase());
    let p2_ascii = ctx.prev2.map(|s| strip_accents(&s.to_lowercase()));
    let mut scored: Vec<(i32, u32, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push_cand = |word: &str, bonus: i32, rank: u32| {
        let lc = word.to_lowercase();
        if lc != prev.to_lowercase() && seen.insert(lc.clone()) {
            scored.push((bonus, rank, word.to_owned()));
        }
    };

    if ctx.after_apostrophe {
        for (idx, word) in builtin_elision_candidates(lang, &p_ascii).iter().enumerate() {
            push_cand(word, -220 + i32::try_from(idx).unwrap_or(10) * 10, u32::try_from(idx).unwrap_or(10));
        }
        return format_ranked_candidates(lex, scored, false, limit);
    }

    // 1. User's own page trigrams & bigrams come first so domain phrases repeat naturally.
    if let Some(ref p2) = p2_ascii
        && let Some(nexts) = ctx.page.trigrams.get(&(p2.clone(), p_ascii.clone()))
    {
        for (word, &cnt) in nexts {
            let bonus = -240 - i32::try_from(cnt).unwrap_or(10).min(8) * 15;
            push_cand(word, bonus, 0);
        }
    }
    if let Some(nexts) = ctx.page.bigrams.get(&p_ascii) {
        for (word, &cnt) in nexts {
            let bonus = -180 - i32::try_from(cnt).unwrap_or(10).min(8) * 12;
            push_cand(word, bonus, 10);
        }
    }

    // 2. Built-in N-gram language model candidates (trigrams then bigrams).
    for (word, bonus) in builtin_next_candidates(lang, p2_ascii.as_deref(), &p_ascii) {
        push_cand(word, bonus, 20);
    }

    // 3. Page topic vocabulary and everyday connectives filtered by morpho-syntax agreement.
    for topic in &ctx.page.words {
        let cnt = ctx.page.topic_counts.get(topic).copied().unwrap_or(1);
        let gram = grammatical_agreement_bonus(lang, &p_ascii, topic);
        if gram < 35 {
            push_cand(topic, 60 + gram - i32::try_from(cnt).unwrap_or(1).min(5) * 10, 50);
        }
    }
    let fallbacks: &[&str] = match lang {
        SpellLang::Fr => &["et", "de", "la", "le", "les", "un", "une", "des", "dans", "pour", "avec", "sur"],
        SpellLang::En => &["the", "to", "a", "and", "of", "in", "for", "with", "is", "are"],
        SpellLang::De => &["und", "der", "die", "das", "in", "mit", "für", "auf"],
        SpellLang::Es => &["y", "de", "la", "el", "en", "con", "para", "un", "una"],
        SpellLang::It => &["e", "di", "la", "il", "in", "con", "per", "un", "una"],
    };
    for (idx, word) in fallbacks.iter().enumerate() {
        let gram = grammatical_agreement_bonus(lang, &p_ascii, word);
        if gram < 35 {
            push_cand(word, 95 + gram + i32::try_from(idx).unwrap_or(10) * 3, 200);
        }
    }

    format_ranked_candidates(lex, scored, false, limit)
}

fn suggest_words(lex: &Lexicon, lang: SpellLang, typed: &str, is_misspelled: bool, ctx: &WordCtx<'_>, limit: usize) -> Vec<String> {
    let lower = typed.to_lowercase();
    let ascii = strip_accents(&lower);
    let min_len = ascii.chars().count();
    let capitalize = typed.chars().next().is_some_and(char::is_uppercase);

    let mut scored: Vec<(i32, u32, usize)> = Vec::new();
    let mut seen_ids: HashSet<usize> = HashSet::new();

    // Fast 1-letter completion mode ("Je v" -> "vais", "veux", "vois"): only high-frequency &
    // contextual words so 1 letter gives instant, clean iPad-style predictions.
    if min_len == 1 {
        let Some(c0) = ascii.chars().next() else { return Vec::new() };
        if let Some(bucket) = lex.prefix1_map.get(&c0) {
            for &id in bucket {
                let entry = &lex.entries[id];
                if entry.lower != lower && entry.ascii.chars().count() > 1 && seen_ids.insert(id) {
                    let freq_score = (i32::try_from(entry.freq_rank).unwrap_or(10_000) / 14).min(180);
                    let accent_bonus = if entry.lower.starts_with(&lower) { 0 } else { 14 };
                    let len_tie = i32::try_from(entry.ascii.chars().count().saturating_sub(1)).unwrap_or(12).min(12);
                    let ctx_bonus = context_probability_bonus(lang, ctx, &entry.lower);
                    scored.push((40 + freq_score + accent_bonus + len_tie + ctx_bonus, entry.freq_rank, id));
                }
            }
        }
        // Also surface domain words already written on the active page even if rare in OS dict.
        for topic in &ctx.page.words {
            if strip_accents(topic).starts_with(&ascii)
                && let Some(ids) = lex.ascii_map.get(&strip_accents(topic))
            {
                for &id in ids {
                    let entry = &lex.entries[id];
                    if entry.lower == *topic && seen_ids.insert(id) {
                        let ctx_bonus = context_probability_bonus(lang, ctx, &entry.lower);
                        scored.push((60 + ctx_bonus, entry.freq_rank, id));
                    }
                }
            }
        }
        let dyn_cands: Vec<(i32, u32, String)> = scored.into_iter().map(|(s, r, id)| (s, r, lex.entries[id].word.clone())).collect();
        return format_ranked_candidates(lex, dyn_cands, capitalize, limit);
    }

    // 1. Exact accent-free matches (highest probability when accents are omitted: "ecole" -> "école").
    if let Some(ids) = lex.ascii_map.get(&ascii) {
        for &id in ids {
            if lex.entries[id].lower != lower && seen_ids.insert(id) {
                let ctx_bonus = context_probability_bonus(lang, ctx, &lex.entries[id].lower);
                scored.push((ctx_bonus, lex.entries[id].freq_rank, id));
            }
        }
    }

    // 2. Probabilistic smartphone prefix completions via O(1) 2-letter index across the entire
    // vocabulary (unigram word frequency + trigram/bigram transition + page topic context).
    let mut a_chars = ascii.chars();
    let mut prefix_hits = 0usize;
    if let (Some(c0), Some(c1)) = (a_chars.next(), a_chars.next())
        && let Some(bucket) = lex.prefix2_map.get(&(c0, c1))
    {
        for &id in bucket {
            let entry = &lex.entries[id];
            if entry.lower == lower || !entry.ascii.starts_with(&ascii) || entry.ascii.chars().count() <= min_len || !seen_ids.insert(id) {
                continue;
            }
            prefix_hits += 1;
            let freq_score = os_word_frequency_score(&entry.ascii, entry.freq_rank);
            let accent_bonus = if entry.lower.starts_with(&lower) { 0 } else { 15 };
            let len_tie = i32::try_from(entry.ascii.chars().count().saturating_sub(min_len)).unwrap_or(12).min(12);
            let ctx_bonus = context_probability_bonus(lang, ctx, &entry.lower);
            let rank_score = 40 + freq_score + accent_bonus + len_tie + ctx_bonus;
            scored.push((rank_score, entry.freq_rank, id));
        }
    }

    // 3. SymSpell 1-deletion neighborhood lookup for typos on frequent words.
    if prefix_hits < limit || is_misspelled {
        let typed_chars: Vec<char> = ascii.chars().collect();
        let mut symspell_candidates = Vec::new();
        if let Some(ids) = lex.delete_map.get(&ascii) {
            symspell_candidates.extend_from_slice(ids);
        }
        if typed_chars.len() >= 3 {
            for skip in 0..typed_chars.len() {
                let del: String = typed_chars.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, &c)| c).collect();
                if let Some(ids) = lex.delete_map.get(&del) {
                    symspell_candidates.extend_from_slice(ids);
                }
                if let Some(ids) = lex.ascii_map.get(&del) {
                    symspell_candidates.extend_from_slice(ids);
                }
            }
        }
        for id in symspell_candidates {
            let entry = &lex.entries[id];
            if entry.lower != lower && seen_ids.insert(id) {
                let dist = damerau_levenshtein(&ascii, &entry.ascii);
                if dist <= 2 {
                    let freq_score = os_word_frequency_score(&entry.ascii, entry.freq_rank);
                    let jw = jaro_winkler_penalty(&ascii, &entry.ascii);
                    let kb = keyboard_typo_penalty(&ascii, &entry.ascii);
                    let ctx_bonus = context_probability_bonus(lang, ctx, &entry.lower);
                    let base = if prefix_hits > 0 { 220 } else { 110 };
                    scored.push((base + freq_score + i32::try_from(dist).unwrap_or(2) * 70 + jw + kb + ctx_bonus, entry.freq_rank, id));
                }
            }
        }
    }

    // 4. Damerau-Levenshtein & keyboard-proximity typo repair over frequent words when needed.
    if is_misspelled && scored.len() < limit {
        let t_chars: Vec<char> = ascii.chars().collect();
        let t_len = t_chars.len();
        let first_t = t_chars.first().copied();
        for (id, entry) in lex.entries.iter().enumerate() {
            if entry.lower == lower || seen_ids.contains(&id) {
                continue;
            }
            if entry.freq_rank >= 10_000 && entry.ascii.chars().next() != first_t {
                continue;
            }
            let e_len = entry.ascii.chars().count();
            if e_len.abs_diff(t_len) > 2 {
                continue;
            }
            let dist = damerau_levenshtein(&ascii, &entry.ascii);
            if dist <= 2 {
                let freq_score = os_word_frequency_score(&entry.ascii, entry.freq_rank);
                let jw = jaro_winkler_penalty(&ascii, &entry.ascii);
                let kb = keyboard_typo_penalty(&ascii, &entry.ascii);
                let ctx_bonus = context_probability_bonus(lang, ctx, &entry.lower);
                let total = 240 + freq_score + i32::try_from(dist).unwrap_or(2) * 80 + jw + kb + ctx_bonus;
                if seen_ids.insert(id) {
                    scored.push((total, entry.freq_rank, id));
                }
            }
        }
    }

    let mut dyn_cands: Vec<(i32, u32, String)> = scored.into_iter().map(|(s, r, id)| (s, r, lex.entries[id].word.clone())).collect();
    if is_misspelled && let Some(dict) = &lex.hunspell {
        let mut sb = Vec::new();
        dict.suggest(typed, &mut sb);
        for (idx, s) in sb.into_iter().enumerate() {
            let s_low = s.to_lowercase();
            if !dyn_cands.iter().any(|(_, _, w)| w.to_lowercase() == s_low) {
                let rank = 10_000_u32.saturating_add(u32::try_from(idx).unwrap_or(100));
                let score = 260_i32.saturating_add(i32::try_from(idx).unwrap_or(20) * 18);
                dyn_cands.push((score, rank, s));
            }
        }
    }
    format_ranked_candidates(lex, dyn_cands, capitalize, limit)
}

/// Level-1 offline frequency prior: uses real built-in rank for top everyday words and estimates
/// everyday frequency for OS Hunspell words by morphology & length rather than alphabetical order.
fn os_word_frequency_score(ascii: &str, freq_rank: u32) -> i32 {
    if freq_rank < 10_000 {
        return (i32::try_from(freq_rank).unwrap_or(10_000) / 14).min(180);
    }
    let len = ascii.chars().count();
    let len_pen = match len {
        4..=9 => 0,
        2 | 3 => 16,
        10..=12 => 15,
        _ => 32,
    };
    let morph_adj = if ascii.ends_with("tion")
        || ascii.ends_with("sion")
        || ascii.ends_with("ment")
        || ascii.ends_with("eur")
        || ascii.ends_with("euse")
        || ascii.ends_with("age")
        || ascii.ends_with("able")
        || ascii.ends_with("ible")
        || ascii.ends_with("ique")
        || ascii.ends_with("oir")
        || ascii.ends_with("er")
        || ascii.ends_with("ir")
    {
        -22
    } else if ascii.contains('-') || ascii.ends_with("ismes") || ascii.ends_with("assiez") || ascii.ends_with("ussiez") || ascii.ends_with("assent") {
        28
    } else {
        0
    };
    (215 + len_pen + morph_adj).max(185)
}

fn builtin_elision_candidates(lang: SpellLang, prev_ascii: &str) -> &'static [&'static str] {
    match lang {
        SpellLang::Fr => match prev_ascii {
            "c" => &["est", "était", "étaient"],
            "j" => &["ai", "aime", "avais", "espère", "écoute", "écris"],
            "l" => &["école", "élève", "exemple", "exercice", "eau", "histoire", "enfant"],
            "d" => &["un", "une", "abord", "accord", "autres", "eau"],
            "n" => &["est", "a", "ai", "avons", "ont", "importe"],
            "s" => &["il", "est", "était", "en", "appelle"],
            "m" => &["a", "est", "en"],
            "t" => &["a", "es", "en", "as"],
            "qu" => &["il", "elle", "on", "un", "une", "à"],
            "aujourd" => &["hui"],
            _ => &[],
        },
        SpellLang::It => match prev_ascii {
            "l" => &["amico", "anno", "ora", "Italia"],
            "c" => &["è", "era"],
            _ => &[],
        },
        _ => &[],
    }
}

/// Smartphone-style trigram + bigram candidate table (`(word, rank_bonus)`).
fn builtin_next_candidates(lang: SpellLang, prev2_ascii: Option<&str>, prev_ascii: &str) -> Vec<(&'static str, i32)> {
    let trigram_words: &'static [&'static str] = match (lang, prev2_ascii, prev_ascii) {
        (SpellLang::Fr, Some("il" | "elle" | "on"), "y") => &["a", "avait", "aura"],
        (SpellLang::Fr, Some("est"), "ce") => &["que", "qui"],
        (SpellLang::Fr, Some("en"), "train") => &["de"],
        (SpellLang::Fr, Some("tout"), "le") => &["monde", "temps", "jour"],
        (SpellLang::Fr, Some("tous"), "les") => &["jours", "élèves", "enfants", "deux", "matins"],
        (SpellLang::Fr, Some("toutes"), "les") => &["pages", "questions", "réponses", "fois"],
        (SpellLang::Fr, Some("a"), "la") => &["maison", "fin", "classe", "plage", "piscine", "montagne"],
        (SpellLang::Fr, Some("dans"), "la") => &["classe", "maison", "chambre", "cuisine", "cour", "forêt"],
        (SpellLang::Fr, Some("dans"), "le") => &["cahier", "cartable", "livre", "tableau", "jardin", "texte"],
        (SpellLang::Fr, Some("sur"), "la") => &["table", "feuille", "page", "ligne", "chaise"],
        (SpellLang::Fr, Some("sur"), "le") => &["tableau", "cahier", "bureau", "sol", "dessin"],
        (SpellLang::Fr, Some("je"), "vais") | (SpellLang::Fr, Some("tu"), "vas") | (SpellLang::Fr, Some("on"), "va") => {
            &["faire", "aller", "voir", "lire", "écrire", "au"]
        }
        (SpellLang::Fr, Some("je"), "suis") | (SpellLang::Fr, Some("tu"), "es") | (SpellLang::Fr, Some("il" | "elle"), "est") => {
            &["content", "heureux", "prêt", "très", "à", "en"]
        }
        (SpellLang::Fr, Some("j"), "ai")
        | (SpellLang::Fr, Some("tu"), "as")
        | (SpellLang::Fr, Some("il" | "elle" | "on"), "a")
        | (SpellLang::Fr, Some("nous"), "avons")
        | (SpellLang::Fr, Some("vous"), "avez")
        | (SpellLang::Fr, Some("ils" | "elles"), "ont") => &["fait", "vu", "fini", "lu", "écrit", "mangé", "trouvé", "compris", "un", "une"],
        (SpellLang::Fr, Some("je"), "veux" | "peux" | "dois") | (SpellLang::Fr, Some("il"), "faut") => {
            &["faire", "lire", "écrire", "aller", "voir", "finir"]
        }
        (SpellLang::Fr, Some("c"), "est") => &["un", "une", "très", "bien", "important", "facile", "le", "la"],
        (SpellLang::Fr, Some("il"), "fait") => &["beau", "chaud", "froid", "attention"],
        (SpellLang::Fr, Some("pour"), "le") => &["cours", "devoir", "contrôle", "travail"],
        (SpellLang::Fr, Some("pour"), "la") => &["classe", "maison", "leçon", "question"],
        (SpellLang::En, Some("i"), "am") => &["going", "happy", "ready", "writing"],
        (SpellLang::En, Some("going" | "want" | "have"), "to") => &["go", "be", "do", "make", "read", "write"],
        _ => &[],
    };
    let mut out = Vec::new();
    for (idx, &w) in trigram_words.iter().enumerate() {
        out.push((w, -210 + i32::try_from(idx).unwrap_or(10) * 8));
    }

    let bigram_words: &'static [&'static str] = match (lang, prev_ascii) {
        (SpellLang::Fr, "je") => &["suis", "vais", "veux", "peux", "dois", "fais", "pense", "aime", "travaille"],
        (SpellLang::Fr, "tu") => &["es", "as", "vas", "veux", "peux", "fais", "aimes"],
        (SpellLang::Fr, "il" | "elle" | "on") => &["est", "a", "va", "fait", "peut", "veut", "faut", "y"],
        (SpellLang::Fr, "nous") => &["sommes", "avons", "allons", "faisons", "pouvons", "voulons"],
        (SpellLang::Fr, "vous") => &["êtes", "avez", "allez", "faites", "pouvez", "voulez"],
        (SpellLang::Fr, "ils" | "elles") => &["sont", "ont", "vont", "font", "peuvent", "veulent"],
        (SpellLang::Fr, "le") => &["cours", "problème", "cahier", "texte", "tableau", "livre", "monde", "chat", "chien"],
        (SpellLang::Fr, "la") => &["classe", "maison", "question", "réponse", "phrase", "page", "leçon", "table"],
        (SpellLang::Fr, "les") => &["élèves", "enfants", "amis", "parents", "devoirs", "exercices", "mots", "nombres"],
        (SpellLang::Fr, "un") => &["peu", "petit", "grand", "exemple", "problème", "exercice", "nombre", "texte"],
        (SpellLang::Fr, "une") => &["question", "réponse", "phrase", "page", "histoire", "leçon", "petite", "grande"],
        (SpellLang::Fr, "des") => &["exercices", "questions", "amis", "élèves", "enfants", "mots", "exemples"],
        (SpellLang::Fr, "mon" | "ton" | "son") => &["cahier", "cartable", "papa", "frère", "ami", "ordinateur", "exercice", "chien"],
        (SpellLang::Fr, "ma" | "ta" | "sa") => &["maman", "maison", "classe", "sœur", "famille", "trousse", "chambre"],
        (SpellLang::Fr, "mes" | "tes" | "ses") => &["amis", "parents", "devoirs", "affaires", "cours", "cahiers", "exercices"],
        (SpellLang::Fr, "au") => &["restaurant", "cinéma", "collège", "lycée", "parc", "tableau", "travail", "magasin"],
        (SpellLang::Fr, "avec") => &["mes", "mon", "ma", "les", "un", "une", "toi", "attention"],
        (SpellLang::Fr, "dans") => &["le", "la", "les", "mon", "ma", "un", "une"],
        (SpellLang::Fr, "pour") => &["le", "la", "les", "faire", "aller", "lire", "écrire", "un", "une"],
        (SpellLang::Fr, "parce") => &["que"],
        (SpellLang::Fr, "bonjour") => &["tout", "papa", "maman", "madame", "monsieur"],
        (SpellLang::Fr, "merci") => &["beaucoup", "bien", "maman", "papa"],
        (SpellLang::Fr, "tres") => &["bien", "bon", "bonne", "grand", "petit", "content", "important"],
        (SpellLang::Fr, "plus") => &["grand", "petit", "tard", "vite", "facile", "souvent"],
        (SpellLang::Fr, "beaucoup") => &["de"],
        (SpellLang::Fr, "aujourd") => &["hui"],
        (SpellLang::En, "i") => &["am", "have", "will", "can", "go", "think", "want"],
        (SpellLang::En, "we" | "they" | "you") => &["are", "have", "can", "will", "do", "go"],
        (SpellLang::En, "to") => &["the", "go", "be", "do", "make", "see", "read", "write"],
        (SpellLang::En, "the") => &["school", "class", "question", "answer", "teacher", "book"],
        (SpellLang::En, "my") => &["homework", "notebook", "teacher", "friend", "family", "school"],
        _ => &[],
    };
    for (idx, &w) in bigram_words.iter().enumerate() {
        if !out.iter().any(|&(existing, _)| existing == w) {
            out.push((w, -155 + i32::try_from(idx).unwrap_or(10) * 8));
        }
    }
    out
}

fn grammatical_agreement_bonus(lang: SpellLang, prev_ascii: &str, candidate_lower: &str) -> i32 {
    match lang {
        SpellLang::Fr => match prev_ascii {
            "les" | "des" | "ces" | "mes" | "tes" | "ses" | "nos" | "vos" | "leurs" | "plusieurs" | "quelques" | "deux" | "trois" | "quatre"
            | "cinq" | "six" | "sept" | "huit" | "neuf" | "dix" | "tous" | "toutes" => {
                if (candidate_lower.ends_with('s') || candidate_lower.ends_with('x')) && candidate_lower.chars().count() >= 3 { -75 } else { 65 }
            }
            "le" | "la" | "un" | "une" | "du" | "au" | "mon" | "ton" | "son" | "ma" | "ta" | "sa" | "ce" | "cet" | "cette" | "chaque" => {
                if (candidate_lower.ends_with('s') || candidate_lower.ends_with('x'))
                    && !matches!(
                        candidate_lower,
                        "temps"
                            | "corps"
                            | "cours"
                            | "pays"
                            | "français"
                            | "anglais"
                            | "fois"
                            | "repas"
                            | "bras"
                            | "dos"
                            | "mois"
                            | "printemps"
                            | "heureux"
                            | "toujours"
                            | "plus"
                            | "très"
                            | "dans"
                    )
                {
                    65
                } else if matches!(prev_ascii, "la" | "une" | "ma" | "ta" | "sa" | "cette")
                    && (candidate_lower.ends_with('e')
                        || candidate_lower.ends_with("tion")
                        || candidate_lower.ends_with("sion")
                        || candidate_lower.ends_with("té"))
                {
                    -45
                } else {
                    -25
                }
            }
            "ai" | "as" | "a" | "avons" | "avez" | "ont" | "avais" | "avait" | "avions" | "aviez" | "avaient" => {
                if matches!(candidate_lower, "fait" | "vu" | "lu" | "fini" | "écrit" | "pris" | "compris" | "dit" | "mis" | "eu")
                    || candidate_lower.ends_with('é')
                    || candidate_lower.ends_with("ée")
                {
                    -65
                } else if candidate_lower.ends_with("er") && candidate_lower.chars().count() >= 5 {
                    35
                } else {
                    0
                }
            }
            "vais" | "vas" | "va" | "allons" | "allez" | "vont" | "peux" | "peut" | "veux" | "veut" | "dois" | "doit" | "faut" | "pour" | "sans"
            | "à" => {
                if candidate_lower.ends_with("er")
                    || candidate_lower.ends_with("ir")
                    || candidate_lower.ends_with("re")
                    || candidate_lower.ends_with("oir")
                {
                    -55
                } else {
                    0
                }
            }
            "nous" => {
                if candidate_lower.ends_with("ons") || matches!(candidate_lower, "sommes" | "avons") {
                    -60
                } else {
                    0
                }
            }
            "vous" => {
                if candidate_lower.ends_with("ez") || matches!(candidate_lower, "êtes" | "avez") {
                    -60
                } else {
                    0
                }
            }
            "ils" | "elles" => {
                if candidate_lower.ends_with("ent") || matches!(candidate_lower, "sont" | "ont" | "vont" | "font") {
                    -55
                } else {
                    0
                }
            }
            "je" | "j" => {
                if candidate_lower.ends_with('e') || candidate_lower.ends_with('s') {
                    -25
                } else {
                    0
                }
            }
            "tu" if candidate_lower.ends_with("es") || candidate_lower.ends_with('s') => -30,
            _ => 0,
        },
        SpellLang::En => match prev_ascii {
            "i" | "we" | "they" | "you"
                if matches!(candidate_lower, "am" | "are" | "have" | "can" | "will" | "do" | "go" | "think" | "know" | "want" | "read" | "write") =>
            {
                -50
            }
            "to" | "can" | "will" | "should" | "must"
                if matches!(candidate_lower, "go" | "be" | "have" | "do" | "make" | "see" | "eat" | "play" | "work" | "read" | "write") =>
            {
                -40
            }
            _ => 0,
        },
        _ => 0,
    }
}

/// Combines page trigrams, page bigrams, topic frequency, built-in N-grams and agreement rules.
fn context_probability_bonus(lang: SpellLang, ctx: &WordCtx<'_>, candidate_lower: &str) -> i32 {
    let mut best = 0i32;
    if let Some(&topic_cnt) = ctx.page.topic_counts.get(candidate_lower) {
        best = best.min(-45 - i32::try_from(topic_cnt).unwrap_or(5).min(5) * 10);
    }
    let Some(prev) = ctx.prev else {
        return best;
    };
    let p_ascii = strip_accents(&prev.to_lowercase());
    let p2_ascii = ctx.prev2.map(|s| strip_accents(&s.to_lowercase()));

    if ctx.after_apostrophe {
        let elisions = builtin_elision_candidates(lang, &p_ascii);
        if let Some(pos) = elisions.iter().position(|&w| w == candidate_lower) {
            return best.min(-220 + i32::try_from(pos).unwrap_or(10) * 10);
        }
    }
    if let Some(ref p2) = p2_ascii
        && let Some(nexts) = ctx.page.trigrams.get(&(p2.clone(), p_ascii.clone()))
        && let Some(&cnt) = nexts.get(candidate_lower)
    {
        best = best.min(-240 - i32::try_from(cnt).unwrap_or(10).min(8) * 15);
    }
    if let Some(nexts) = ctx.page.bigrams.get(&p_ascii)
        && let Some(&cnt) = nexts.get(candidate_lower)
    {
        best = best.min(-180 - i32::try_from(cnt).unwrap_or(10).min(8) * 12);
    }
    for (w, bonus) in builtin_next_candidates(lang, p2_ascii.as_deref(), &p_ascii) {
        if w == candidate_lower {
            best = best.min(bonus);
            break;
        }
    }
    let gram = grammatical_agreement_bonus(lang, &p_ascii, candidate_lower);
    if best == 0 {
        gram
    } else if gram > 0 {
        // Explicit phrase/page matches win even if a heuristic disagrees; slight nudge otherwise.
        best
    } else {
        best.min(gram)
    }
}

/// Jaro-Winkler similarity converted into a compact integer ranking penalty `(0..=50)`.
fn jaro_winkler_penalty(a: &str, b: &str) -> i32 {
    let sim = jaro_winkler(a, b);
    let penalty = ((1.0 - sim.clamp(0.0, 1.0)) * 50.0).round();
    if penalty.is_finite() { penalty as i32 } else { 25 }
}

fn jaro_winkler(a: &str, b: &str) -> f32 {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.is_empty() && bc.is_empty() {
        return 1.0;
    }
    if ac.is_empty() || bc.is_empty() {
        return 0.0;
    }
    let match_dist = (ac.len().max(bc.len()) / 2).saturating_sub(1);
    let mut a_matched = vec![false; ac.len()];
    let mut b_matched = vec![false; bc.len()];
    let mut matches = 0usize;
    for (i, &ch) in ac.iter().enumerate() {
        let start = i.saturating_sub(match_dist);
        let end = (i + match_dist + 1).min(bc.len());
        for j in start..end {
            if !b_matched[j] && ch == bc[j] {
                a_matched[i] = true;
                b_matched[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut k = 0usize;
    let mut transpositions = 0usize;
    for (i, &matched) in a_matched.iter().enumerate() {
        if !matched {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if ac[i] != bc[k] {
            transpositions += 1;
        }
        k += 1;
    }
    let m = matches as f32;
    let t = (transpositions as f32) / 2.0;
    let jaro = ((m / ac.len() as f32) + (m / bc.len() as f32) + ((m - t) / m)) / 3.0;
    let prefix = ac.iter().zip(&bc).take(4).take_while(|(x, y)| x == y).count() as f32;
    (jaro + prefix * 0.1 * (1.0 - jaro)).min(1.0)
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out = String::new();
            for uc in first.to_uppercase() {
                out.push(uc);
            }
            out.push_str(chars.as_str());
            out
        }
    }
}

/// Small penalty reward when two same-length words differ by an adjacent AZERTY/QWERTY key slip.
fn keyboard_typo_penalty(a: &str, b: &str) -> i32 {
    let pos = |c: char| -> Option<(i32, i32)> {
        match c {
            'a' => Some((0, 0)),
            'z' => Some((1, 0)),
            'e' => Some((2, 0)),
            'r' => Some((3, 0)),
            't' => Some((4, 0)),
            'y' => Some((5, 0)),
            'u' => Some((6, 0)),
            'i' => Some((7, 0)),
            'o' => Some((8, 0)),
            'p' => Some((9, 0)),
            'q' => Some((0, 1)),
            's' => Some((1, 1)),
            'd' => Some((2, 1)),
            'f' => Some((3, 1)),
            'g' => Some((4, 1)),
            'h' => Some((5, 1)),
            'j' => Some((6, 1)),
            'k' => Some((7, 1)),
            'l' => Some((8, 1)),
            'm' => Some((9, 1)),
            'w' => Some((0, 2)),
            'x' => Some((1, 2)),
            'c' => Some((2, 2)),
            'v' => Some((3, 2)),
            'b' => Some((4, 2)),
            'n' => Some((5, 2)),
            _ => None,
        }
    };
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len() != bc.len() {
        return 12;
    }
    let diffs: Vec<(char, char)> = ac.into_iter().zip(bc).filter(|(x, y)| x != y).collect();
    if diffs.len() == 1
        && let (Some((x0, y0)), Some((x1, y1))) = (pos(diffs[0].0), pos(diffs[0].1))
        && (x0 - x1).abs() <= 1
        && (y0 - y1).abs() <= 1
    {
        return 0;
    }
    18
}

fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate().take(n + 1) {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate().take(m + 1) {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[i - 2][j - 2] + cost);
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

fn builtin_words(lang: SpellLang) -> &'static [&'static str] {
    match lang {
        SpellLang::Fr => &[
            "le",
            "la",
            "les",
            "un",
            "une",
            "des",
            "du",
            "de",
            "au",
            "aux",
            "et",
            "ou",
            "où",
            "est",
            "sont",
            "être",
            "avoir",
            "suis",
            "es",
            "sommes",
            "êtes",
            "ai",
            "as",
            "avons",
            "avez",
            "ont",
            "faire",
            "fait",
            "fais",
            "faisons",
            "faites",
            "faisait",
            "font",
            "prendre",
            "prends",
            "prend",
            "prenons",
            "prenez",
            "prennent",
            "connaître",
            "connais",
            "connaît",
            "progrès",
            "fils",
            "aller",
            "vais",
            "va",
            "allons",
            "allez",
            "vont",
            "voir",
            "vois",
            "voit",
            "lire",
            "lis",
            "lit",
            "écrire",
            "écris",
            "écrit",
            "école",
            "élève",
            "élèves",
            "classe",
            "cours",
            "exercice",
            "exercices",
            "leçon",
            "leçons",
            "devoir",
            "devoirs",
            "cahier",
            "cartable",
            "livre",
            "livres",
            "page",
            "pages",
            "ligne",
            "lignes",
            "mot",
            "mots",
            "phrase",
            "phrases",
            "texte",
            "question",
            "questions",
            "réponse",
            "réponses",
            "exemple",
            "exemples",
            "tableau",
            "tableaux",
            "titre",
            "sous-titre",
            "mathématiques",
            "français",
            "histoire",
            "géographie",
            "sciences",
            "anglais",
            "allemand",
            "espagnol",
            "italien",
            "addition",
            "soustraction",
            "multiplication",
            "division",
            "calcul",
            "calculer",
            "nombre",
            "nombres",
            "chiffre",
            "centaines",
            "dizaines",
            "unités",
            "milliers",
            "retenue",
            "résultat",
            "problème",
            "problèmes",
            "solution",
            "mesure",
            "longueur",
            "mètre",
            "kilomètre",
            "centimètre",
            "millimètre",
            "gramme",
            "kilogramme",
            "litre",
            "heure",
            "minute",
            "triangle",
            "carré",
            "rectangle",
            "cercle",
            "droite",
            "segment",
            "angle",
            "périmètre",
            "aire",
            "fraction",
            "pourcentage",
            "bonjour",
            "merci",
            "beaucoup",
            "toujours",
            "souvent",
            "parfois",
            "jamais",
            "aujourd",
            "hui",
            "demain",
            "hier",
            "maintenant",
            "ensuite",
            "pendant",
            "avant",
            "après",
            "pourquoi",
            "comment",
            "quand",
            "combien",
            "quel",
            "quelle",
            "quels",
            "quelles",
            "parce",
            "que",
            "qui",
            "quoi",
            "dont",
            "dans",
            "sur",
            "sous",
            "avec",
            "sans",
            "pour",
            "par",
            "vers",
            "maison",
            "ordinateur",
            "clavier",
            "souris",
            "écran",
            "tablette",
            "stylo",
            "crayon",
            "gomme",
            "règle",
            "compas",
            "trousse",
            "feuille",
            "dossier",
            "fichier",
            "image",
            "photo",
            "caméra",
            "audio",
            "vidéo",
            "dicter",
            "dictée",
            "important",
            "remarque",
            "attention",
            "définition",
            "résumé",
            "synthèse",
            "conclusion",
            "introduction",
            "brouillon",
            "bateau",
            "château",
            "oiseau",
            "cadeau",
            "eau",
            "feutre",
            "couleur",
            "rouge",
            "bleu",
            "vert",
            "jaune",
            "orange",
            "violet",
            "noir",
            "blanc",
            "gris",
            "rose",
            "marron",
            "grand",
            "grande",
            "petit",
            "petite",
            "nouveau",
            "nouvelle",
            "bon",
            "bonne",
            "très",
            "bien",
            "aussi",
            "mais",
            "donc",
            "car",
            "ni",
            "cependant",
            "toutefois",
            "encore",
            "déjà",
            "famille",
            "enfant",
            "enfants",
            "parent",
            "parents",
            "professeur",
            "maître",
            "maîtresse",
            "camarade",
            "ami",
            "amis",
            "amie",
            "amies",
            "papa",
            "maman",
            "parents",
            "frère",
            "sœur",
            "restaurant",
            "repas",
            "dîner",
            "déjeuner",
            "manger",
            "mange",
            "jouer",
            "joue",
            "travailler",
            "travaille",
            "vacances",
            "voiture",
            "téléphone",
            "vélo",
            "train",
            "avion",
            "bus",
            "jardin",
            "parc",
            "cinéma",
            "piscine",
            "plage",
            "magasin",
            "chambre",
            "salon",
            "cuisine",
            "table",
            "chaise",
            "fenêtre",
            "porte",
            "gâteau",
            "pain",
            "chocolat",
            "fraise",
            "pomme",
            "banane",
            "chien",
            "chat",
            "cheval",
            "oiseau",
            "poisson",
            "rue",
            "petit",
            "petite",
            "grand",
            "grande",
            "ensemble",
            "heureux",
            "heureuse",
            "content",
            "contente",
            "éléphant",
            "animaux",
            "animal",
            "plante",
            "nature",
            "terre",
            "soleil",
            "lune",
            "étoile",
            "planète",
            "océan",
            "montagne",
            "rivière",
            "forêt",
            "ville",
            "village",
            "pays",
            "France",
            "Europe",
            "monde",
            "temps",
            "année",
            "mois",
            "semaine",
            "jour",
            "médecin",
            "hôpital",
            "musique",
            "sport",
            "dessin",
            "peinture",
            "lecture",
            "écriture",
            "orthographe",
            "grammaire",
            "conjugaison",
            "vocabulaire",
            "verbe",
            "nom",
            "adjectif",
            "pronom",
            "adverbe",
            "préposition",
            "sujet",
            "complément",
            "présent",
            "passé",
            "futur",
            "imparfait",
            "féminin",
            "masculin",
            "singulier",
            "pluriel",
            "syllabe",
            "syllabes",
            "voyelle",
            "consonne",
            "accent",
            "majuscule",
            "point",
            "virgule",
            "guillemets",
            "parenthèses",
            "première",
            "deuxième",
            "troisième",
            "quatrième",
            "cinquième",
            "sixième",
            "septième",
            "huitième",
            "neuvième",
            "comprendre",
            "apprendre",
            "retenir",
            "réciter",
            "expliquer",
            "vérifier",
            "corriger",
            "souligner",
            "surligner",
            "entourer",
            "relier",
            "compléter",
            "recopier",
            "chercher",
            "trouver",
            "penser",
            "croire",
            "savoir",
            "pouvoir",
            "vouloir",
            "devoir",
            "falloir",
            "prendre",
            "donner",
            "mettre",
            "laisser",
            "garder",
            "ouvrir",
            "fermer",
            "commencer",
            "terminer",
            "parler",
            "écouter",
            "regarder",
            "observer",
            "dessiner",
            "mesurer",
            "comparer",
            "ranger",
            "classer",
            "trier",
            "je",
            "tu",
            "il",
            "elle",
            "on",
            "nous",
            "vous",
            "ils",
            "elles",
            "mon",
            "ma",
            "mes",
            "ton",
            "ta",
            "tes",
            "son",
            "sa",
            "ses",
            "notre",
            "nos",
            "votre",
            "vos",
            "leur",
            "leurs",
            "ce",
            "cet",
            "cette",
            "ces",
            "tout",
            "toute",
            "tous",
            "toutes",
            "plus",
            "moins",
            "autant",
            "trop",
            "assez",
            "peu",
            "ici",
            "là",
            "alors",
            "ainsi",
            "comme",
            "même",
            "autre",
            "autres",
            "en",
            "y",
            "a",
            "deux",
            "trois",
            "quatre",
            "cinq",
            "six",
            "sept",
            "huit",
            "neuf",
            "dix",
            "avais",
            "avait",
            "avions",
            "aviez",
            "avaient",
            "aura",
            "était",
            "étaient",
            "peux",
            "peut",
            "peuvent",
            "pouvons",
            "pouvez",
            "veux",
            "veut",
            "voulons",
            "voulez",
            "veulent",
            "dois",
            "doit",
            "faut",
            "viens",
            "vient",
            "pense",
            "aime",
            "aimes",
            "travaille",
            "travailles",
            "joue",
            "joues",
            "espère",
            "écoute",
            "vu",
            "lu",
            "fini",
            "mangé",
            "trouvé",
            "compris",
            "parlé",
            "aimé",
            "travaillé",
            "joué",
            "dessiné",
            "calculé",
            "dit",
            "mis",
            "pris",
            "eu",
            "beau",
            "chaud",
            "froid",
            "facile",
            "difficile",
            "prêt",
            "tard",
            "vite",
            "monde",
            "train",
            "fois",
            "fin",
            "cour",
            "sol",
            "bureau",
            "besoin",
            "contrôle",
            "travail",
            "matin",
            "matins",
            "accord",
            "abord",
            "importe",
            "appelle",
            "lycée",
            "collège",
            "enfant",
            "chienne",
            "toi",
            "lui",
            "elle",
            "nous",
            "pleut",
            "affaires",
        ],
        SpellLang::En => &[
            "the",
            "be",
            "to",
            "of",
            "and",
            "a",
            "in",
            "that",
            "have",
            "I",
            "it",
            "for",
            "not",
            "on",
            "with",
            "he",
            "as",
            "you",
            "do",
            "at",
            "this",
            "but",
            "his",
            "by",
            "from",
            "they",
            "we",
            "say",
            "her",
            "she",
            "or",
            "an",
            "will",
            "my",
            "one",
            "all",
            "would",
            "there",
            "their",
            "what",
            "so",
            "up",
            "out",
            "if",
            "about",
            "who",
            "get",
            "which",
            "go",
            "me",
            "when",
            "make",
            "can",
            "like",
            "time",
            "no",
            "just",
            "him",
            "know",
            "take",
            "people",
            "into",
            "year",
            "your",
            "good",
            "some",
            "could",
            "them",
            "see",
            "other",
            "than",
            "then",
            "now",
            "look",
            "only",
            "come",
            "its",
            "over",
            "think",
            "also",
            "back",
            "after",
            "use",
            "two",
            "how",
            "our",
            "work",
            "first",
            "well",
            "way",
            "even",
            "new",
            "want",
            "because",
            "any",
            "these",
            "give",
            "day",
            "most",
            "us",
            "school",
            "student",
            "students",
            "teacher",
            "lesson",
            "homework",
            "notebook",
            "page",
            "question",
            "answer",
            "example",
            "reading",
            "writing",
            "spelling",
            "grammar",
            "sentence",
            "paragraph",
            "word",
            "words",
            "mathematics",
            "science",
            "history",
            "geography",
            "English",
            "computer",
            "keyboard",
            "pencil",
            "ruler",
            "table",
            "addition",
            "subtraction",
            "multiplication",
            "division",
            "fraction",
            "problem",
            "solution",
            "number",
            "numbers",
            "hundred",
            "thousand",
            "elephant",
            "family",
            "friend",
            "always",
            "usually",
            "sometimes",
            "never",
            "today",
            "tomorrow",
            "yesterday",
            "important",
            "remember",
            "understand",
            "explain",
            "complete",
            "check",
            "correct",
            "underline",
            "highlight",
            "hello",
            "thanks",
            "please",
            "sorry",
            "welcome",
            "great",
            "small",
            "large",
            "different",
            "same",
            "before",
            "between",
        ],
        SpellLang::De => &[
            "der",
            "die",
            "das",
            "ein",
            "eine",
            "einen",
            "einem",
            "einer",
            "und",
            "oder",
            "aber",
            "denn",
            "weil",
            "dass",
            "ich",
            "du",
            "er",
            "sie",
            "es",
            "wir",
            "ihr",
            "Sie",
            "mein",
            "dein",
            "sein",
            "unser",
            "euer",
            "haben",
            "hat",
            "sind",
            "ist",
            "war",
            "wird",
            "werden",
            "kann",
            "können",
            "muss",
            "müssen",
            "soll",
            "will",
            "machen",
            "macht",
            "gehen",
            "geht",
            "sehen",
            "sieht",
            "lesen",
            "liest",
            "schreiben",
            "schreibt",
            "lernen",
            "lernt",
            "verstehen",
            "Schule",
            "Schüler",
            "Schülerin",
            "Lehrer",
            "Lehrerin",
            "Klasse",
            "Unterricht",
            "Hausaufgabe",
            "Hausaufgaben",
            "Übung",
            "Übungen",
            "Heft",
            "Buch",
            "Seite",
            "Wort",
            "Wörter",
            "Satz",
            "Sätze",
            "Text",
            "Frage",
            "Antwort",
            "Beispiel",
            "Tabelle",
            "Mathematik",
            "Deutsch",
            "Englisch",
            "Französisch",
            "Geschichte",
            "Erdkunde",
            "Biologie",
            "Physik",
            "Chemie",
            "Addition",
            "Subtraktion",
            "Multiplikation",
            "Division",
            "Rechnung",
            "Ergebnis",
            "Aufgabe",
            "Lösung",
            "Zahl",
            "Zahlen",
            "Hunderter",
            "Zehner",
            "Einer",
            "Tausender",
            "Übertrag",
            "heute",
            "morgen",
            "gestern",
            "immer",
            "oft",
            "manchmal",
            "nie",
            "warum",
            "wie",
            "wann",
            "wer",
            "was",
            "wo",
            "wichtig",
            "Achtung",
            "Hinweis",
            "Zusammenfassung",
            "Hallo",
            "Danke",
            "Bitte",
            "gut",
            "groß",
            "klein",
            "neu",
            "alt",
            "richtig",
            "falsch",
            "Computer",
            "Tastatur",
        ],
        SpellLang::Es => &[
            "el",
            "la",
            "los",
            "las",
            "un",
            "una",
            "unos",
            "unas",
            "y",
            "o",
            "pero",
            "porque",
            "que",
            "quien",
            "cual",
            "como",
            "donde",
            "cuando",
            "cuanto",
            "yo",
            "tú",
            "él",
            "ella",
            "nosotros",
            "vosotros",
            "ellos",
            "ellas",
            "ser",
            "es",
            "son",
            "estar",
            "está",
            "están",
            "tener",
            "tiene",
            "tienen",
            "hacer",
            "hace",
            "ir",
            "va",
            "ver",
            "leer",
            "escribir",
            "escuela",
            "colegio",
            "alumno",
            "alumna",
            "profesor",
            "profesora",
            "clase",
            "lección",
            "deberes",
            "ejercicio",
            "cuaderno",
            "libro",
            "página",
            "línea",
            "palabra",
            "palabras",
            "frase",
            "oración",
            "texto",
            "pregunta",
            "respuesta",
            "ejemplo",
            "tabla",
            "título",
            "matemáticas",
            "lengua",
            "historia",
            "geografía",
            "ciencias",
            "inglés",
            "español",
            "suma",
            "resta",
            "multiplicación",
            "división",
            "cálculo",
            "número",
            "números",
            "centenas",
            "decenas",
            "unidades",
            "resultado",
            "problema",
            "solución",
            "hoy",
            "mañana",
            "ayer",
            "siempre",
            "nunca",
            "ahora",
            "después",
            "antes",
            "hola",
            "gracias",
            "por",
            "favor",
            "para",
            "con",
            "sin",
            "sobre",
            "entre",
            "importante",
            "recuerda",
            "atención",
            "bueno",
            "grande",
            "pequeño",
            "nuevo",
            "correcto",
            "ordenador",
            "teclado",
            "ratón",
            "lápiz",
            "regla",
            "familia",
        ],
        SpellLang::It => &[
            "il",
            "lo",
            "la",
            "i",
            "gli",
            "le",
            "un",
            "uno",
            "una",
            "e",
            "o",
            "ma",
            "perché",
            "che",
            "chi",
            "quale",
            "come",
            "dove",
            "quando",
            "quanto",
            "io",
            "tu",
            "lui",
            "lei",
            "noi",
            "voi",
            "loro",
            "essere",
            "è",
            "sono",
            "avere",
            "ha",
            "hanno",
            "fare",
            "fa",
            "andare",
            "va",
            "vedere",
            "leggere",
            "scrivere",
            "scuola",
            "studente",
            "studentessa",
            "insegnante",
            "classe",
            "lezione",
            "compiti",
            "esercizio",
            "quaderno",
            "libro",
            "pagina",
            "riga",
            "parola",
            "parole",
            "frase",
            "testo",
            "domanda",
            "risposta",
            "esempio",
            "tabella",
            "titolo",
            "matematica",
            "italiano",
            "storia",
            "geografia",
            "scienze",
            "inglese",
            "addizione",
            "sottrazione",
            "moltiplicazione",
            "divisione",
            "calcolo",
            "numero",
            "numeri",
            "centinaia",
            "decine",
            "unità",
            "risultato",
            "problema",
            "soluzione",
            "oggi",
            "domani",
            "ieri",
            "sempre",
            "mai",
            "adesso",
            "prima",
            "dopo",
            "ciao",
            "grazie",
            "prego",
            "per",
            "con",
            "senza",
            "tra",
            "fra",
            "importante",
            "attenzione",
            "ricorda",
            "buono",
            "grande",
            "piccolo",
            "nuovo",
            "corretto",
            "computer",
            "tastiera",
            "famiglia",
        ],
    }
}

/// Spelling, homophone or syntax error reported by the on-demand orthography checker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrammarIssue {
    pub char_range: (usize, usize),
    pub original: String,
    pub message: String,
    pub suggestions: Vec<String>,
}

fn same_clause_next<'a>(chars: &[char], tokens: &'a [(usize, usize, String)], idx: usize) -> Option<&'a str> {
    let next_idx = idx + 1;
    let next = tokens.get(next_idx)?;
    is_clause_gap(chars, tokens[idx].1, next.0).then_some(next.2.as_str())
}

fn match_casing(original: &str, replacement: &str) -> String {
    if original.chars().next().is_some_and(char::is_uppercase) && !replacement.chars().next().is_some_and(char::is_uppercase) {
        capitalize_first(replacement)
    } else {
        replacement.to_owned()
    }
}

fn french_phonetic_key(s: &str) -> String {
    let ascii = strip_accents(&s.to_lowercase());
    let chars: Vec<char> = ascii.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0usize;
    while i < chars.len() {
        let rem: String = chars[i..].iter().collect();
        if rem.starts_with("eau") {
            out.push('o');
            i += 3;
            continue;
        }
        if rem.starts_with("au") {
            out.push('o');
            i += 2;
            continue;
        }
        if rem.starts_with("ai") || rem.starts_with("ei") {
            out.push('e');
            i += 2;
            continue;
        }
        if rem.starts_with("ph") {
            out.push('f');
            i += 2;
            continue;
        }
        if rem.starts_with("qu") {
            out.push('k');
            i += 2;
            continue;
        }
        let ch = chars[i];
        let nxt = chars.get(i + 1).copied();
        let mapped = match ch {
            'z' => 's',
            'c' if matches!(nxt, Some('e' | 'i' | 'y')) => 's',
            'c' => 'k',
            other => other,
        };
        if !out.ends_with(mapped) {
            out.push(mapped);
        }
        i += 1;
    }
    out
}

fn push_if_known(lex: &Lexicon, lang: SpellLang, cand: &str, out: &mut Vec<String>) {
    if is_known_word(lex, lang, cand) && !out.iter().any(|w| w == cand) {
        out.push(cand.to_owned());
    }
}

fn french_phonetic_rewrites(lex: &Lexicon, lang: SpellLang, lower: &str) -> Vec<String> {
    if lang != SpellLang::Fr {
        return Vec::new();
    }
    let mut out = Vec::new();
    let fixed: &[&str] = match lower {
        "féson" | "feson" | "fésons" | "fesons" | "fezon" | "fezons" | "faison" | "faisont" => &["faisons"],
        "fésait" | "fesait" | "fézait" | "fezait" => &["faisait"],
        "fésais" | "fesais" => &["faisais"],
        "fésaient" | "fesaient" => &["faisaient"],
        "fésant" | "fesant" => &["faisant"],
        "pren" => &["prends", "prend", "prenez"],
        "prent" => &["prend", "prends"],
        "compren" => &["comprends", "comprend"],
        "appren" => &["apprends", "apprend"],
        "repren" => &["reprends", "reprend"],
        "connait" | "connais" => &["connaît"],
        "progrè" | "progrés" | "progre" => &["progrès"],
        "succè" | "succés" => &["succès"],
        "procè" | "procés" => &["procès"],
        "accè" | "accés" => &["accès"],
        "excè" | "excés" => &["excès"],
        "aprè" | "aprés" => &["après"],
        "trè" | "trés" => &["très"],
        "toujour" => &["toujours"],
        "jamai" => &["jamais"],
        "beaucou" | "bocou" | "bokou" | "bcp" => &["beaucoup"],
        "aujourdhui" => &["aujourd'hui"],
        _ => &[],
    };
    for &cand in fixed {
        push_if_known(lex, lang, cand, &mut out);
    }

    // Cédille manquante devant a / o / u (`lecons` -> `leçons`, `garcon` -> `garçon`).
    for (idx, ch) in lower.char_indices() {
        if ch == 'c'
            && let Some(after) = lower.get(idx + ch.len_utf8()..)
            && after.starts_with(['a', 'o', 'u', 'â', 'ô'])
        {
            let cand = format!("{}ç{}", &lower[..idx], after);
            push_if_known(lex, lang, &cand, &mut out);
            if !cand.ends_with(['s', 'x', 'z']) {
                push_if_known(lex, lang, &format!("{cand}s"), &mut out);
            }
        }
    }

    // Lettre finale muette ou terminaison oubliée (`pren` -> `prends`, `progrè` -> `progrès`).
    for suf in ["s", "ds", "ts", "t", "d", "x", "e", "es"] {
        push_if_known(lex, lang, &format!("{lower}{suf}"), &mut out);
    }
    if let Some(stem) = lower.strip_suffix('é') {
        for rep in ["ès", "et", "er", "ez", "ée", "és"] {
            push_if_known(lex, lang, &format!("{stem}{rep}"), &mut out);
        }
    }
    if let Some(stem) = lower.strip_suffix('è') {
        for rep in ["ès", "et", "ai", "ais", "ait"] {
            push_if_known(lex, lang, &format!("{stem}{rep}"), &mut out);
        }
    }
    if let Some(rest) = lower.strip_prefix("fés").or_else(|| lower.strip_prefix("fes")) {
        for suf in ["", "s", "t"] {
            push_if_known(lex, lang, &format!("fais{rest}{suf}"), &mut out);
        }
    }
    out
}

fn context_fit_bonus(lang: SpellLang, cand: &str, prev: Option<&str>, next: Option<&str>) -> i32 {
    if lang != SpellLang::Fr {
        return 0;
    }
    let c_low = cand.to_lowercase();
    let mut bonus = 0i32;
    if let Some(p) = prev {
        match p {
            "je" | "j" => {
                if matches!(
                    c_low.as_str(),
                    "après"
                        | "très"
                        | "dans"
                        | "sans"
                        | "sous"
                        | "vers"
                        | "pour"
                        | "avec"
                        | "mais"
                        | "plus"
                        | "moins"
                        | "trop"
                        | "alors"
                        | "toujours"
                        | "jamais"
                        | "dehors"
                        | "dedans"
                        | "depuis"
                        | "pendant"
                ) {
                    bonus += 100;
                } else if c_low.ends_with("ons") || c_low.ends_with("ez") || c_low.ends_with("ent") {
                    bonus += 140;
                } else if c_low.ends_with("ds") || c_low.ends_with("ts") || c_low.ends_with('s') || c_low.ends_with('x') || c_low.ends_with('e') {
                    bonus -= 220;
                } else if c_low.ends_with("er") || c_low.ends_with("ir") || c_low.ends_with("re") || c_low.ends_with("ant") {
                    bonus += 120;
                }
            }
            "tu" => {
                if c_low.ends_with('s') || c_low.ends_with('x') {
                    bonus -= 220;
                } else if c_low.ends_with("er") || c_low.ends_with("ent") {
                    bonus += 100;
                }
            }
            "il" | "elle" | "on" => {
                if c_low.ends_with('t') || c_low.ends_with('d') || c_low.ends_with('e') || c_low.ends_with('a') {
                    bonus -= 190;
                } else if c_low.ends_with("ons") || c_low.ends_with("ez") || c_low.ends_with("ent") {
                    bonus += 90;
                }
            }
            "nous" => {
                if c_low.ends_with("ons") {
                    bonus -= 260;
                } else {
                    bonus += 100;
                }
            }
            "vous" => {
                if c_low.ends_with("ez") || c_low.ends_with("tes") {
                    bonus -= 260;
                } else {
                    bonus += 100;
                }
            }
            "ils" | "elles" => {
                if c_low.ends_with("ent") || c_low.ends_with("ont") {
                    bonus -= 250;
                } else {
                    bonus += 90;
                }
            }
            "les" | "des" | "ces" | "mes" | "tes" | "ses" | "nos" | "vos" | "leurs" | "dé" | "lé" | "mé" | "té" | "sé" | "quelques" | "plusieurs"
            | "deux" | "trois" | "quatre" | "cinq" => {
                if c_low.ends_with('s') || c_low.ends_with('x') {
                    bonus -= 180;
                } else {
                    bonus += 95;
                }
            }
            "le" | "la" | "l" | "un" | "une" | "mon" | "ton" | "son" | "ce" | "cet" | "cette" | "chaque" => {
                let invariant = matches!(
                    c_low.as_str(),
                    "fils" | "progrès" | "succès" | "avis" | "prix" | "corps" | "cours" | "temps" | "mois" | "bras" | "dos" | "pays"
                );
                if c_low.ends_with('s') && !invariant {
                    bonus += 85;
                } else {
                    bonus -= 85;
                }
            }
            _ => {}
        }
    }
    if let Some(n) = next {
        let n_low = n.to_lowercase();
        if n_low.ends_with('s') && n_low.chars().count() >= 4 && matches!(c_low.as_str(), "les" | "des" | "ces" | "mes" | "tes" | "ses") {
            bonus -= 60;
        }
    }
    bonus
}

/// Rank spelling corrections for a *finished* word in its grammatical context (no long-word autocomplete).
fn suggest_corrections_for_word(lex: &Lexicon, lang: SpellLang, typed: &str, prev: Option<&str>, next: Option<&str>, limit: usize) -> Vec<String> {
    let lower = typed.to_lowercase();
    let ascii = strip_accents(&lower);
    let typed_phon = french_phonetic_key(&lower);
    let t_len = ascii.chars().count();
    let capitalize = typed.chars().next().is_some_and(char::is_uppercase);

    let rewrites = french_phonetic_rewrites(lex, lang, &lower);
    let rewrite_rank: HashMap<String, usize> = rewrites.iter().enumerate().map(|(idx, w)| (w.to_lowercase(), idx)).collect();
    let mut raw: Vec<String> = rewrites;
    let mut seen: HashSet<String> = raw.iter().map(|s| s.to_lowercase()).collect();

    if let Some(ids) = lex.ascii_map.get(&ascii) {
        for &id in ids {
            let e = &lex.entries[id];
            if e.lower != lower && seen.insert(e.lower.clone()) {
                raw.push(e.word.clone());
            }
        }
    }

    let mut a_chars = ascii.chars();
    if let (Some(c0), Some(c1)) = (a_chars.next(), a_chars.next())
        && let Some(bucket) = lex.prefix2_map.get(&(c0, c1))
    {
        for &id in bucket {
            let e = &lex.entries[id];
            let e_len = e.ascii.chars().count();
            if e.lower != lower && e.ascii.starts_with(&ascii) && e_len <= t_len + 2 && seen.insert(e.lower.clone()) {
                raw.push(e.word.clone());
            }
        }
    }

    let mut symspell_ids: Vec<usize> = Vec::new();
    if let Some(ids) = lex.delete_map.get(&ascii) {
        symspell_ids.extend_from_slice(ids);
    }
    let typed_chars: Vec<char> = ascii.chars().collect();
    if typed_chars.len() >= 3 {
        for skip in 0..typed_chars.len() {
            let del: String = typed_chars.iter().enumerate().filter(|&(idx, _)| idx != skip).map(|(_, &c)| c).collect();
            if let Some(ids) = lex.delete_map.get(&del) {
                symspell_ids.extend_from_slice(ids);
            }
            if let Some(ids) = lex.ascii_map.get(&del) {
                symspell_ids.extend_from_slice(ids);
            }
        }
    }
    for id in symspell_ids {
        let e = &lex.entries[id];
        if e.lower != lower && damerau_levenshtein(&ascii, &e.ascii) <= 2 && seen.insert(e.lower.clone()) {
            raw.push(e.word.clone());
        }
    }

    let first_t = typed_chars.first().copied();
    for e in &lex.entries {
        if e.lower == lower || e.ascii.chars().count().abs_diff(t_len) > 2 {
            continue;
        }
        if e.freq_rank >= 10_000 && e.ascii.chars().next() != first_t {
            continue;
        }
        if damerau_levenshtein(&ascii, &e.ascii) <= 2 && seen.insert(e.lower.clone()) {
            raw.push(e.word.clone());
        }
    }

    if let Some(dict) = &lex.hunspell {
        let mut sb = Vec::new();
        dict.suggest(typed, &mut sb);
        for s in sb {
            let sl = s.to_lowercase();
            if sl != lower && !sl.contains(' ') && seen.insert(sl) {
                raw.push(s);
            }
        }
    }

    let mut scored: Vec<(i32, u32, String)> = Vec::with_capacity(raw.len());
    for cand in raw {
        let c_low = cand.to_lowercase();
        let c_ascii = strip_accents(&c_low);
        let dist = damerau_levenshtein(&ascii, &c_ascii);
        if dist > 3 && french_phonetic_key(&c_low).trim_end_matches('s') != typed_phon.trim_end_matches('s') {
            continue;
        }
        let freq_rank = lex
            .ascii_map
            .get(&c_ascii)
            .and_then(|ids| ids.iter().find(|&&id| lex.entries[id].lower == c_low).map(|&id| lex.entries[id].freq_rank))
            .unwrap_or(5_000);
        let freq_score = os_word_frequency_score(&c_ascii, freq_rank);
        let phon_bonus =
            if lang == SpellLang::Fr && french_phonetic_key(&c_low).trim_end_matches('s') == typed_phon.trim_end_matches('s') { -90 } else { 0 };
        let rewrite_bonus = rewrite_rank.get(&c_low).map_or(0, |&idx| (-280_i32).saturating_add(i32::try_from(idx).unwrap_or(5) * 35));
        let prefix_bonus = if c_ascii.starts_with(&ascii) {
            -70
        } else if c_ascii.chars().next() != first_t {
            55
        } else {
            0
        };
        let accent_penalty = if c_ascii == ascii { 0 } else { 18 };
        let jw = jaro_winkler_penalty(&ascii, &c_ascii);
        let kb = keyboard_typo_penalty(&ascii, &c_ascii);
        let ctx_bonus = context_fit_bonus(lang, &cand, prev, next);
        let dist_score = i32::try_from(dist).unwrap_or(3) * 90;
        let total = dist_score + accent_penalty + freq_score + jw + kb + phon_bonus + rewrite_bonus + prefix_bonus + ctx_bonus;
        scored.push((total, freq_rank, cand));
    }

    format_ranked_candidates(lex, scored, capitalize, limit)
}

fn explain_orthography_issue(orig: &str, best: &str, prev: Option<&str>, lang: SpellLang) -> String {
    if lang != SpellLang::Fr {
        return format!("Orthographe : « {orig} » doit s'écrire « {best} ».");
    }
    let o_low = orig.to_lowercase();
    let b_low = best.to_lowercase();
    if o_low == b_low {
        return format!("Majuscule attendue en début de phrase : écrivez « {best} ».");
    }
    let o_ascii = strip_accents(&o_low);
    let b_ascii = strip_accents(&b_low);
    if o_ascii == b_ascii {
        if (o_low.contains("ca") || o_low.contains("co") || o_low.contains("cu"))
            && (b_low.contains("ça") || b_low.contains("ço") || b_low.contains("çu"))
        {
            return format!("Cédille manquante : devant « a », « o » ou « u », il faut écrire « ç » pour garder le son [s] (« {best} »).");
        }
        if !o_low.contains('î') && b_low.contains('î') {
            return format!("Accent circonflexe sur le « î » : devant un « t », les verbes en « -aître » prennent un « î » (« {best} »).");
        }
        if !o_low.contains('é') && b_low.contains('é') {
            return format!("Accent aigu manquant sur le « é » : on écrit « {best} ».");
        }
        if (!o_low.contains('è') && b_low.contains('è')) || (!o_low.contains('ê') && b_low.contains('ê')) {
            return format!("Accent grave ou circonflexe manquant : on écrit « {best} ».");
        }
        return format!("Accent manquant ou différent : ce mot s'écrit « {best} ».");
    }
    if prev == Some("nous") && b_low.ends_with("ons") {
        if b_low.contains("ai") && !o_low.contains("ai") {
            return format!("Son [ə] et conjugaison : avec « nous », le verbe s'écrit « ai » (comme « fais- ») et finit par « -ons » (« {best} »).");
        }
        return format!("Accord sujet-verbe : avec « nous », la terminaison est toujours « -ons » (« {best} »).");
    }
    if prev == Some("vous") && b_low.ends_with("ez") {
        return format!("Accord sujet-verbe : avec « vous », la terminaison est « -ez » (« {best} »).");
    }
    if matches!(prev, Some("je" | "j"))
        && (b_low == format!("{o_low}ds") || b_low == format!("{o_low}ts") || b_low == format!("{o_low}s") || b_low == format!("{o_low}x"))
    {
        return format!("Lettre finale muette & conjugaison : avec « je », le verbe conjugué prend sa consonne finale muette (« {best} »).");
    }
    if matches!(prev, Some("tu")) && b_low.ends_with('s') && o_ascii == b_ascii.trim_end_matches('s') {
        return format!("Accord sujet-verbe : avec « tu », le verbe se termine par « -s » (« {best} »).");
    }
    if b_ascii == format!("{o_ascii}s") || b_ascii == format!("{o_ascii}x") {
        let silent = b_ascii.chars().last().unwrap_or('s');
        let accent_note = if o_low != o_ascii || strip_accents(&b_low) != b_low { " et son accent" } else { "" };
        return format!("Lettre finale muette : « {best} » prend un « {silent} » muet à la fin{accent_note} (« {best} »).");
    }
    if b_low == format!("{o_low}t") || b_low == format!("{o_low}d") {
        let silent = b_low.chars().last().unwrap_or('t');
        return format!("Consonne finale muette : il manque le « {silent} » à la fin de « {best} ».");
    }
    if o_ascii.starts_with("fes") && b_low.starts_with("fais") {
        return format!("Graphie du son [ə] dans « faire » : on écrit « ai » et non « é/e » (« {best} »).");
    }
    format!("Orthographe d'usage : « {orig} » ne s'écrit pas ainsi ; on écrit « {best} ».")
}

fn is_sentence_start_gap(chars: &[char], from: usize, to: usize) -> bool {
    if from >= to || to > chars.len() {
        return false;
    }
    let gap = &chars[from..to];
    let has_end_punct = gap.iter().any(|c| matches!(c, '.' | '!' | '?' | '…'));
    let has_ws = gap.iter().any(|c| c.is_whitespace());
    has_end_punct && has_ws
}

fn clause_has_3sg_subject_before(chars: &[char], tokens: &[(usize, usize, String)], idx: usize) -> bool {
    let mut start = idx;
    while start > 0 && is_clause_gap(chars, tokens[start - 1].1, tokens[start].0) {
        start -= 1;
    }
    if idx < start + 2 {
        return false;
    }
    for pos in start..idx - 1 {
        let w = tokens[pos].2.to_lowercase();
        if matches!(w.as_str(), "il" | "elle" | "on") {
            return true;
        }
        if pos + 1 < idx - 1
            && matches!(
                w.as_str(),
                "mon" | "ton" | "son" | "notre" | "votre" | "leur" | "le" | "la" | "un" | "une" | "ce" | "cet" | "cette" | "chaque"
            )
            && !matches!(tokens[pos + 1].2.to_lowercase().as_str(), "qui" | "que" | "dont" | "où" | "et" | "ou")
        {
            return true;
        }
    }
    false
}

/// Scans a whole block or selection for lexical misspellings, DYS homophones (`a/à`, `et/est`,
/// `son/sont`, `ce/se`, `ou/où`, `ces/ses`, `dé/des`), `-er/-é` verb errors and agreement/conjugation mistakes.
pub fn scan_orthography_and_syntax(text: &str, lang: SpellLang) -> Vec<GrammarIssue> {
    let chars: Vec<char> = text.chars().collect();
    let tokens = word_chars_with_ranges(text);
    let lex = lexicon(lang);
    let mut issues: Vec<GrammarIssue> = Vec::new();
    let mut push = |start: usize, end: usize, orig: &str, msg: String, raw_suggs: Vec<String>| {
        if start >= end || raw_suggs.is_empty() {
            return;
        }
        if issues.iter().any(|existing| existing.char_range.0 < end && start < existing.char_range.1) {
            return;
        }
        let preserve_cap = !msg.starts_with("Majuscule");
        let suggestions: Vec<String> =
            raw_suggs.into_iter().take(MAX_SUGGESTIONS).map(|s| if preserve_cap { match_casing(orig, &s) } else { s }).collect();
        issues.push(GrammarIssue { char_range: (start, end), original: orig.to_owned(), message: msg, suggestions });
    };

    for (i, (start, end, word)) in tokens.iter().enumerate() {
        let lower = word.to_lowercase();
        let prev = same_clause_prev(&chars, &tokens, i).map(str::to_lowercase);
        let prev2 = i.checked_sub(1).and_then(|pi| same_clause_prev(&chars, &tokens, pi)).map(str::to_lowercase);
        let next = same_clause_next(&chars, &tokens, i).map(str::to_lowercase);

        // 0. Majuscule oubliée après un point, un point d'exclamation ou d'interrogation.
        if i > 0
            && is_sentence_start_gap(&chars, tokens[i - 1].1, *start)
            && word.chars().next().is_some_and(char::is_lowercase)
            && !matches!(tokens[i - 1].2.to_lowercase().as_str(), "etc" | "cf" | "ex" | "m" | "mme" | "dr")
            && is_known_word(lex, lang, &lower)
        {
            let cap = capitalize_first(word);
            let msg = format!("Majuscule en début de phrase : après un point, la phrase commence toujours par une majuscule (« {cap} »).");
            push(*start, *end, word, msg, vec![cap]);
            continue;
        }

        if lang == SpellLang::Fr {
            // 1. Homophones grammaticaux & confusions phonétiques courantes.
            match lower.as_str() {
                "à" if matches!(prev.as_deref(), Some("il" | "elle" | "on")) => {
                    push(*start, *end, word, "Verbe avoir sans accent : « a » (on peut dire « avait »).".into(), vec!["a".into()]);
                    continue;
                }
                "a" if matches!(
                    prev.as_deref(),
                    Some(
                        "va" | "vais"
                            | "vas"
                            | "allons"
                            | "allez"
                            | "vont"
                            | "commence"
                            | "commencé"
                            | "apprend"
                            | "appris"
                            | "continue"
                            | "continué"
                            | "sert"
                            | "aide"
                            | "aidé"
                            | "jusqu"
                            | "grâce"
                            | "grace"
                    )
                ) =>
                {
                    push(*start, *end, word, "Préposition avec accent : « à » (on ne peut pas dire « avait »).".into(), vec!["à".into()]);
                    continue;
                }
                "et" if matches!(prev.as_deref(), Some("il" | "elle" | "on" | "c" | "ce" | "qui"))
                    && matches!(
                        next.as_deref(),
                        Some(
                            "un" | "une"
                                | "le"
                                | "la"
                                | "les"
                                | "très"
                                | "tres"
                                | "trop"
                                | "plus"
                                | "bien"
                                | "bon"
                                | "bonne"
                                | "grand"
                                | "grande"
                                | "petit"
                                | "petite"
                                | "content"
                                | "contente"
                                | "heureux"
                                | "heureuse"
                                | "important"
                                | "facile"
                                | "difficile"
                                | "prêt"
                                | "pret"
                                | "en"
                                | "dans"
                                | "parti"
                                | "fait"
                        )
                    ) =>
                {
                    push(*start, *end, word, "Verbe être : « est » (on peut dire « était »).".into(), vec!["est".into()]);
                    continue;
                }
                "est"
                    if (matches!(prev.as_deref(), Some("toi" | "moi" | "lui" | "papa" | "maman"))
                        && matches!(next.as_deref(), Some("moi" | "toi" | "lui" | "elle" | "papa" | "maman")))
                        || (!matches!(
                            prev.as_deref(),
                            None | Some(
                                "qu" | "que"
                                    | "qui"
                                    | "c"
                                    | "ce"
                                    | "il"
                                    | "elle"
                                    | "on"
                                    | "où"
                                    | "ou"
                                    | "quand"
                                    | "quel"
                                    | "quelle"
                                    | "comment"
                                    | "pourquoi"
                            )
                        ) && matches!(next.as_deref(), Some("je" | "j" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles"))
                            && tokens.get(i + 1).is_some_and(|nxt| !chars[*end..nxt.0].contains(&'-'))) =>
                {
                    push(
                        *start,
                        *end,
                        word,
                        "Homophone « est / et » : devant un sujet (« nous / tu / il... »), écrivez la conjonction « et » (on ne peut pas dire « était »).".into(),
                        vec!["et".into()],
                    );
                    continue;
                }
                "dé" if !matches!(
                    prev.as_deref(),
                    Some("un" | "le" | "ce" | "mon" | "ton" | "son" | "quel" | "chaque" | "du" | "au" | "petit" | "grand" | "gros")
                ) && next.as_deref().is_some_and(|n| {
                    !matches!(
                        n,
                        "à" | "de"
                            | "du"
                            | "en"
                            | "et"
                            | "ou"
                            | "qui"
                            | "que"
                            | "dont"
                            | "où"
                            | "pour"
                            | "sans"
                            | "avec"
                            | "sur"
                            | "par"
                            | "est"
                            | "sont"
                            | "était"
                            | "a"
                            | "va"
                            | "fait"
                            | "tombe"
                            | "tombé"
                            | "roule"
                    )
                }) =>
                {
                    push(
                        *start,
                        *end,
                        word,
                        "Homophone phonétique « dé / des » : devant un nom pluriel, l'article indéfini s'écrit « des » (et non « dé » comme le dé à jouer).".into(),
                        vec!["des".into()],
                    );
                    continue;
                }
                "lé" | "mé" | "té" | "sé" if next.is_some() => {
                    let rep = match lower.as_str() {
                        "lé" => "les",
                        "mé" => "mes",
                        "té" => "tes",
                        _ => "ses",
                    };
                    push(
                        *start,
                        *end,
                        word,
                        format!("Déterminant pluriel écrit au son : on écrit « {rep} » (et non « {lower} »)."),
                        vec![rep.into()],
                    );
                    continue;
                }
                "fis"
                    if matches!(
                        prev.as_deref(),
                        Some("mon" | "ton" | "son" | "notre" | "votre" | "leur" | "le" | "un" | "ce" | "cet" | "du" | "au")
                    ) && !matches!(
                        prev2.as_deref(),
                        Some("je" | "j" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles" | "qu" | "qui" | "que" | "ne" | "n")
                    ) =>
                {
                    let p = prev.as_deref().unwrap_or("mon");
                    push(
                        *start,
                        *end,
                        word,
                        format!(
                            "Lettre muette / confusion : après « {p} », le verbe « fis » est impossible ; le nom s'écrit « fils » avec un « l » muet."
                        ),
                        vec!["fils".into(), "fil".into()],
                    );
                    continue;
                }
                "connait" | "reconnait" | "parait" | "apparait" | "disparait" | "nait" | "renait" | "plait" | "deplait" => {
                    if let Some(stem) = lower.strip_suffix("ait") {
                        let fixed = format!("{stem}aît");
                        push(
                            *start,
                            *end,
                            word,
                            format!("Accent circonflexe sur le « î » : devant un « t », les verbes en « -aître » prennent un « î » (« {fixed} »)."),
                            vec![fixed],
                        );
                        continue;
                    }
                }
                "ces"
                    if prev.is_some()
                        && !matches!(
                            prev.as_deref(),
                            Some(
                                "de" | "pour" | "avec" | "sans" | "dans" | "sur" | "sous" | "par" | "vers" | "entre" | "chez" | "et" | "ou" | "mais"
                            )
                        )
                        && !matches!(next.as_deref(), Some("ci" | "là" | "la" | "derniers" | "dernières"))
                        && !matches!(tokens.get(i + 2).map(|t| t.2.to_lowercase()).as_deref(), Some("ci" | "là" | "la" | "qui" | "que" | "dont"))
                        && next.as_deref().is_some_and(|n| n.ends_with(['s', 'x']) || n == "lecon")
                        && clause_has_3sg_subject_before(&chars, &tokens, i) =>
                {
                    push(
                        *start,
                        *end,
                        word,
                        "Homophone « ces / ses » : écrivez le possessif « ses » (= les siennes, celles du sujet singulier) plutôt que le démonstratif « ces ».".into(),
                        vec!["ses".into()],
                    );
                    continue;
                }
                "son" if matches!(prev.as_deref(), Some("ils" | "elles")) => {
                    push(*start, *end, word, "Verbe être au pluriel : « sont » (on peut dire « étaient »).".into(), vec!["sont".into()]);
                    continue;
                }
                "sont" if matches!(prev.as_deref(), Some("dans" | "avec" | "pour" | "sur" | "par" | "de" | "sous" | "vers" | "sans")) => {
                    push(*start, *end, word, "Déterminant possessif : « son » (ex. son cahier).".into(), vec!["son".into()]);
                    continue;
                }
                "ce" if matches!(prev.as_deref(), Some("il" | "elle" | "on" | "ils" | "elles"))
                    && !matches!(next.as_deref(), Some("que" | "qui" | "dont")) =>
                {
                    push(*start, *end, word, "Pronom réfléchi devant un verbe : « se ».".into(), vec!["se".into()]);
                    continue;
                }
                "se" if matches!(prev.as_deref(), Some("dans" | "pour" | "avec" | "sur" | "sous" | "pendant")) => {
                    push(*start, *end, word, "Déterminant démonstratif devant un nom : « ce ».".into(), vec!["ce".into()]);
                    continue;
                }
                "ou" if matches!(
                    prev.as_deref(),
                    Some("moment" | "jour" | "endroit" | "lieu" | "pays" | "ville" | "classe" | "cahier" | "maison" | "école" | "ecole")
                ) || matches!(next.as_deref(), Some("est" | "sont" | "va" | "vas" | "allons" | "allez" | "se" | "habite" | "habites")) =>
                {
                    push(*start, *end, word, "Mot indiquant le lieu ou le moment : « où » avec accent.".into(), vec!["où".into()]);
                    continue;
                }
                _ => {}
            }

            // 2. Confusion infinitif `-er` / participe passé `-é`.
            let is_aux = matches!(prev.as_deref(), Some("ai" | "as" | "a" | "avons" | "avez" | "ont" | "avais" | "avait"))
                || (prev.as_deref() == Some("à") && matches!(prev2.as_deref(), Some("il" | "elle" | "on")));
            if is_aux
                && lower.ends_with("er")
                && lower.chars().count() >= 5
                && !matches!(
                    lower.as_str(),
                    "cahier"
                        | "papier"
                        | "dossier"
                        | "panier"
                        | "métier"
                        | "atelier"
                        | "escalier"
                        | "premier"
                        | "dernier"
                        | "entier"
                        | "léger"
                        | "hiver"
                )
                && let Some(stem) = lower.strip_suffix("er")
            {
                let pp = format!("{stem}é");
                if lex.exact_lower.contains(&pp) {
                    push(*start, *end, word, "Participe passé en « -é » après l'auxiliaire avoir.".into(), vec![pp]);
                    continue;
                }
            }
            if matches!(
                prev.as_deref(),
                Some(
                    "pour"
                        | "sans"
                        | "de"
                        | "à"
                        | "vais"
                        | "vas"
                        | "va"
                        | "allons"
                        | "allez"
                        | "vont"
                        | "peux"
                        | "peut"
                        | "veux"
                        | "veut"
                        | "dois"
                        | "doit"
                        | "faut"
                )
            ) && lower.ends_with('é')
                && lower.chars().count() >= 4
                && !matches!(lower.as_str(), "café" | "thé" | "côté" | "été" | "clé" | "bébé" | "lycée" | "musée" | "blé")
                && let Some(stem) = lower.strip_suffix('é')
            {
                let inf = format!("{stem}er");
                if lex.exact_lower.contains(&inf) {
                    push(*start, *end, word, "Infinitif en « -er » après une préposition ou un verbe conjugué.".into(), vec![inf]);
                    continue;
                }
            }

            // 3. Accords déterminant + nom (singulier / pluriel).
            let det_plural = matches!(
                prev.as_deref(),
                Some(
                    "les"
                        | "des"
                        | "ces"
                        | "mes"
                        | "tes"
                        | "ses"
                        | "nos"
                        | "vos"
                        | "leurs"
                        | "plusieurs"
                        | "quelques"
                        | "deux"
                        | "trois"
                        | "quatre"
                        | "cinq"
                )
            ) && !matches!(prev2.as_deref(), Some("je" | "j" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles"));
            if det_plural && word.chars().count() >= 3 && !lower.ends_with(['s', 'x', 'z']) && is_known_word(lex, lang, &lower) {
                let cand_s = format!("{lower}s");
                let cand_x = format!("{lower}x");
                let cand_aux = lower.strip_suffix("al").map(|s| format!("{s}aux"));
                let plural = if lex.exact_lower.contains(&cand_s) {
                    Some(cand_s)
                } else if lex.exact_lower.contains(&cand_x) {
                    Some(cand_x)
                } else {
                    cand_aux.filter(|c| lex.exact_lower.contains(c))
                };
                if let Some(p_word) = plural {
                    push(
                        *start,
                        *end,
                        word,
                        "Accord au pluriel : après un déterminant pluriel, le nom prend « -s » (ou « -x »).".into(),
                        vec![p_word],
                    );
                    continue;
                }
            }
            let det_sing = matches!(
                prev.as_deref(),
                Some("le" | "la" | "un" | "une" | "mon" | "ton" | "son" | "ma" | "ta" | "sa" | "ce" | "cet" | "cette" | "chaque")
            ) && !matches!(prev2.as_deref(), Some("je" | "j" | "tu" | "il" | "elle" | "on" | "nous" | "vous" | "ils" | "elles"));
            if det_sing
                && word.chars().count() >= 4
                && lower.ends_with('s')
                && !lower.ends_with("ss")
                && !matches!(
                    lower.as_str(),
                    "temps"
                        | "corps"
                        | "cours"
                        | "pays"
                        | "français"
                        | "anglais"
                        | "fois"
                        | "repas"
                        | "bras"
                        | "dos"
                        | "mois"
                        | "printemps"
                        | "souris"
                        | "tapis"
                        | "radis"
                        | "brebis"
                        | "bois"
                        | "plus"
                        | "très"
                        | "dans"
                        | "vers"
                        | "sous"
                        | "mais"
                        | "alors"
                        | "toujours"
                        | "fils"
                        | "os"
                        | "as"
                        | "avis"
                        | "prix"
                        | "nez"
                        | "gaz"
                        | "poids"
                        | "secours"
                        | "concours"
                        | "discours"
                        | "progrès"
                        | "succès"
                        | "procès"
                        | "excès"
                        | "accès"
                        | "héros"
                        | "chaos"
                        | "virus"
                        | "bus"
                        | "ours"
                        | "gens"
                )
                && let Some(stem) = lower.strip_suffix('s')
                && lex.exact_lower.contains(stem)
            {
                push(
                    *start,
                    *end,
                    word,
                    "Accord au singulier : après un déterminant singulier, on retire le « -s » final.".into(),
                    vec![stem.to_owned()],
                );
                continue;
            }

            // 4. Accords sujet + verbe (présent usuel).
            match prev.as_deref() {
                Some("nous") if lower.ends_with(['e', 's', 't']) && lower.chars().count() >= 4 => {
                    let base = lower.strip_suffix("ent").or_else(|| lower.strip_suffix("es")).or_else(|| lower.strip_suffix('e')).unwrap_or(&lower);
                    let inf = format!("{base}er");
                    if let Some(stem_c) = base.strip_suffix('c')
                        && (lex.exact_lower.contains(&format!("{stem_c}cer")) || lex.exact_lower.contains(&format!("{stem_c}çons")))
                    {
                        push(
                            *start,
                            *end,
                            word,
                            "Conjugaison : avec « nous », le verbe se termine par « -ons » (et « c » devient « ç » devant « o »).".into(),
                            vec![format!("{stem_c}çons")],
                        );
                        continue;
                    }
                    let target = if base.ends_with('g') { format!("{base}eons") } else { format!("{base}ons") };
                    if lex.exact_lower.contains(&inf) || lex.exact_lower.contains(&target) {
                        push(*start, *end, word, "Conjugaison : avec le pronom « nous », le verbe se termine par « -ons ».".into(), vec![target]);
                        continue;
                    }
                }
                Some("vous") if (lower.ends_with('e') || lower.ends_with("es") || lower.ends_with("ent")) && lower.chars().count() >= 4 => {
                    let stem = lower.strip_suffix("ent").or_else(|| lower.strip_suffix("es")).or_else(|| lower.strip_suffix('e')).unwrap_or(&lower);
                    let inf = format!("{stem}er");
                    let ez = format!("{stem}ez");
                    if lex.exact_lower.contains(&inf) || lex.exact_lower.contains(&ez) {
                        push(*start, *end, word, "Conjugaison : avec le pronom « vous », le verbe se termine par « -ez ».".into(), vec![ez]);
                        continue;
                    }
                }
                Some("tu")
                    if lower.ends_with('e')
                        && lower.chars().count() >= 4
                        && (lex.exact_lower.contains(&format!("{lower}s"))
                            || lower.strip_suffix('e').is_some_and(|b| lex.exact_lower.contains(&format!("{b}er")))) =>
                {
                    push(
                        *start,
                        *end,
                        word,
                        "Conjugaison : avec le pronom « tu », le verbe se termine par « -s ».".into(),
                        vec![format!("{lower}s")],
                    );
                    continue;
                }
                Some("ils" | "elles") if lower.ends_with('e') && lower.chars().count() >= 4 => {
                    let ent = format!("{lower}nt");
                    let inf_ok = lower.strip_suffix('e').is_some_and(|b| lex.exact_lower.contains(&format!("{b}er")));
                    if inf_ok || lex.exact_lower.contains(&ent) {
                        push(
                            *start,
                            *end,
                            word,
                            "Conjugaison au pluriel : avec « ils / elles », le verbe se termine par « -ent ».".into(),
                            vec![ent],
                        );
                        continue;
                    }
                }
                _ => {}
            }
        } else if lang == SpellLang::En {
            if lower == "a"
                && let Some(nxt) = next.as_deref()
                && nxt.starts_with(['a', 'e', 'i', 'o', 'u'])
                && !matches!(nxt, "one" | "once" | "user" | "use" | "university" | "uniform")
            {
                push(*start, *end, word, "Use « an » before a vowel sound.".into(), vec!["an".into()]);
                continue;
            }
            if matches!(prev.as_deref(), Some("you" | "we" | "they")) && lower == "is" {
                push(*start, *end, word, "Use « are » with « you / we / they ».".into(), vec!["are".into()]);
                continue;
            }
            if matches!(prev.as_deref(), Some("he" | "she" | "it")) && lower == "are" {
                push(*start, *end, word, "Use « is » with « he / she / it ».".into(), vec!["is".into()]);
                continue;
            }
        }

        // 5. Faute d'orthographe lexicale (mot absent du dictionnaire), classée selon le contexte grammatical.
        // Ne pas signaler les noms propres en milieu de phrase.
        let is_mid_sentence_proper = word.chars().next().is_some_and(char::is_uppercase) && prev.is_some();
        if word.chars().count() >= 2 && !is_mid_sentence_proper && !is_known_word(lex, lang, &lower) {
            let suggestions = suggest_corrections_for_word(lex, lang, word, prev.as_deref(), next.as_deref(), MAX_SUGGESTIONS);
            if let Some(best) = suggestions.first() {
                let msg = explain_orthography_issue(word, best, prev.as_deref(), lang);
                push(*start, *end, word, msg, suggestions);
            }
        }
    }

    issues
}

/// Pure-Rust JSON extractor for on-demand LanguageTool responses (`{"matches":[...]}`).
pub fn parse_languagetool_matches(json: &str, text: &str) -> Vec<GrammarIssue> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let Some(matches_key) = json.find("\"matches\"") else {
        return out;
    };
    let Some(arr_rel) = json[matches_key..].find('[') else {
        return out;
    };
    let bytes = json.as_bytes();
    let mut i = matches_key + arr_rel + 1;
    let mut in_str = false;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut obj_start: Option<usize> = None;

    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0
                        && let Some(start) = obj_start.take()
                        && let Some(obj) = json.get(start..=i)
                        && let Some(issue) = parse_lt_match_obj(obj, &chars)
                    {
                        out.push(issue);
                    }
                }
            }
            b']' if depth == 0 => break,
            _ => {}
        }
        i += 1;
    }
    out
}

fn token_before(chars: &[char], offset: usize) -> Option<String> {
    let mut i = offset.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    let end = i;
    while i > 0 && chars[i - 1].is_alphabetic() {
        i -= 1;
    }
    (i < end).then(|| chars[i..end].iter().collect::<String>().to_lowercase())
}

fn token_after(chars: &[char], offset: usize) -> Option<String> {
    let mut i = offset.min(chars.len());
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < chars.len() && chars[i].is_alphabetic() {
        i += 1;
    }
    (start < i).then(|| chars[start..i].iter().collect::<String>().to_lowercase())
}

fn parse_lt_match_obj(obj: &str, chars: &[char]) -> Option<GrammarIssue> {
    let offset = extract_json_usize(obj, "\"offset\"")?;
    let length = extract_json_usize(obj, "\"length\"")?;
    let lo = offset.min(chars.len());
    let hi = (offset + length).min(chars.len());
    if lo >= hi {
        return None;
    }
    let original: String = chars[lo..hi].iter().collect();
    let raw_msg = extract_json_string(obj, "\"message\"").unwrap_or_else(|| "Suggestion grammaticale".into());
    let mut suggestions = extract_lt_replacements(obj);
    if suggestions.is_empty() {
        return None;
    }
    let message = if raw_msg.starts_with("Faute de frappe") {
        let prev = token_before(chars, lo);
        let next = token_after(chars, hi);
        let lex = lexicon(SpellLang::Fr);
        let mut ranked = suggest_corrections_for_word(lex, SpellLang::Fr, &original, prev.as_deref(), next.as_deref(), MAX_SUGGESTIONS);
        for s in suggestions {
            if ranked.len() < MAX_SUGGESTIONS && !ranked.iter().any(|r| r.eq_ignore_ascii_case(&s)) {
                ranked.push(s);
            }
        }
        suggestions = ranked;
        suggestions.first().map(|best| explain_orthography_issue(&original, best, prev.as_deref(), SpellLang::Fr)).unwrap_or(raw_msg)
    } else {
        raw_msg
    };
    Some(GrammarIssue { char_range: (lo, hi), original, message, suggestions })
}

fn extract_json_usize(obj: &str, key: &str) -> Option<usize> {
    let idx = obj.find(key)? + key.len();
    let rest = obj.get(idx..)?.trim_start();
    let after_colon = rest.strip_prefix(':')?.trim_start();
    let digits: String = after_colon.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn extract_json_string(obj: &str, key: &str) -> Option<String> {
    let idx = obj.find(key)? + key.len();
    let rest = obj.get(idx..)?.trim_start().strip_prefix(':')?.trim_start();
    let inner = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(code) = u32::from_str_radix(&hex, 16)
                        && let Some(uc) = char::from_u32(code)
                    {
                        out.push(uc);
                    }
                }
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

fn extract_lt_replacements(obj: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(rep_pos) = obj.find("\"replacements\"") else {
        return out;
    };
    let Some(arr_pos) = obj[rep_pos..].find('[') else {
        return out;
    };
    let mut slice = &obj[rep_pos + arr_pos + 1..];
    if let Some(end_bracket) = slice.find(']') {
        slice = &slice[..end_bracket];
    }
    while let Some(v_pos) = slice.find("\"value\"") {
        if let Some(val) = extract_json_string(&slice[v_pos..], "\"value\"")
            && !val.trim().is_empty()
            && !out.contains(&val)
        {
            out.push(val);
            if out.len() >= MAX_SUGGESTIONS {
                break;
            }
        }
        slice = &slice[v_pos + 7..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_five_languages_from_os_tags() {
        assert_eq!(SpellLang::from_tag("fr_FR.UTF-8"), Some(SpellLang::Fr));
        assert_eq!(SpellLang::from_tag("en-US"), Some(SpellLang::En));
        assert_eq!(SpellLang::from_tag("de_DE"), Some(SpellLang::De));
        assert_eq!(SpellLang::from_tag("es-ES"), Some(SpellLang::Es));
        assert_eq!(SpellLang::from_tag("it_IT"), Some(SpellLang::It));
    }

    #[test]
    fn french_probabilistic_prediction_fixes_accents_typos_and_context_in_3_words() {
        // Accent fix: "ecole" -> "école"
        let accent = analyze_word_at_caret("à ecole", 7, SpellLang::Fr).expect("analysis");
        assert!(accent.is_misspelled);
        assert!(accent.suggestions.iter().any(|s| s == "école"), "suggestions={:?}", accent.suggestions);

        // Smartphone prefix + context probability: "Je vais au rest" -> "restaurant"
        let rest = analyze_word_at_caret("Je vais au rest", 15, SpellLang::Fr).expect("rest prediction");
        assert!(rest.suggestions.len() <= MAX_SUGGESTIONS, "capped to 3 words: {:?}", rest.suggestions);
        assert_eq!(rest.suggestions.first().map(String::as_str), Some("restaurant"), "suggestions={:?}", rest.suggestions);

        // Plural agreement from preceding word: "avec mes am" -> "amis"
        let amis = analyze_word_at_caret("avec mes am", 11, SpellLang::Fr).expect("plural context prediction");
        assert!(amis.suggestions.len() <= MAX_SUGGESTIONS, "capped to 3 words: {:?}", amis.suggestions);
        assert_eq!(amis.suggestions.first().map(String::as_str), Some("amis"), "suggestions={:?}", amis.suggestions);

        // Typo repair: "le btaeau" / "mon bato" or "ordi" -> "ordinateur"
        let pred = analyze_word_at_caret("mon ordi", 8, SpellLang::Fr).expect("analysis");
        assert!(pred.suggestions.len() <= MAX_SUGGESTIONS, "capped to 3 words: {:?}", pred.suggestions);
        assert!(pred.suggestions.iter().any(|s| s == "ordinateur"), "suggestions={:?}", pred.suggestions);
    }

    #[test]
    fn multilingual_spellcheck_detects_completed_errors_and_suggests_repairs() {
        let bad_en = check_completed_words("I go to schol now", Some(17), SpellLang::En);
        assert!(!bad_en.is_empty(), "schol should be flagged in English");

        let fix_de = analyze_word_at_caret("die Shule", 9, SpellLang::De).expect("analysis");
        assert!(fix_de.suggestions.iter().any(|s| s == "Schule"), "suggestions={:?}", fix_de.suggestions);

        let fix_es = analyze_word_at_caret("la escuel", 9, SpellLang::Es).expect("analysis");
        assert!(fix_es.suggestions.iter().any(|s| s == "escuela"), "suggestions={:?}", fix_es.suggestions);

        let fix_it = analyze_word_at_caret("la scuol", 8, SpellLang::It).expect("analysis");
        assert!(fix_it.suggestions.iter().any(|s| s == "scuola"), "suggestions={:?}", fix_it.suggestions);
    }

    #[test]
    fn apostrophe_elisions_and_markup_do_not_corrupt_word_ranges() {
        let res = analyze_word_at_caret("l'ecole", 7, SpellLang::Fr).expect("elided word");
        assert_eq!(res.word, "ecole");
        assert_eq!(res.char_range, (2, 7));

        let ok = check_completed_words("**bonjour** monde ", Some(18), SpellLang::Fr);
        assert!(ok.is_empty(), "markdown markers should not cause false misspellings: {ok:?}");
    }

    #[test]
    fn ipad_style_next_word_single_letter_and_page_context_predictions_work() {
        // Next-word trigram/bigram prediction right after space:
        let il_y = analyze_word_at_caret("Il y ", 5, SpellLang::Fr).expect("next word after Il y");
        assert!(il_y.word.is_empty());
        assert_eq!(il_y.char_range, (5, 5));
        assert!(!il_y.is_misspelled);
        assert_eq!(il_y.suggestions.first().map(String::as_str), Some("a"), "suggestions={:?}", il_y.suggestions);

        let je_vais = analyze_word_at_caret("Je vais ", 8, SpellLang::Fr).expect("next word after Je vais");
        assert!(je_vais.suggestions.iter().any(|w| w == "faire"), "suggestions={:?}", je_vais.suggestions);

        let nous_avons = analyze_word_at_caret("Nous avons ", 11, SpellLang::Fr).expect("next word after Nous avons");
        assert!(nous_avons.suggestions.iter().any(|w| w == "fait"), "suggestions={:?}", nous_avons.suggestions);

        // Elision right after apostrophe:
        let cest = analyze_word_at_caret("c'", 2, SpellLang::Fr).expect("elision prediction");
        assert_eq!(cest.suggestions.first().map(String::as_str), Some("est"), "suggestions={:?}", cest.suggestions);

        // 1-letter context-sensitive completion:
        let je_v = analyze_word_at_caret("Je v", 4, SpellLang::Fr).expect("single letter after Je");
        assert!(!je_v.is_misspelled);
        assert_eq!(je_v.suggestions.first().map(String::as_str), Some("vais"), "suggestions={:?}", je_v.suggestions);

        // Dynamic page topic learning:
        let page_ctx = "La photosynthèse nourrit la plante. Mon animal préféré est le crocodile.";
        let topic = analyze_word_at_caret_with_context("le c", 4, SpellLang::Fr, page_ctx).expect("page topic prediction");
        assert!(topic.suggestions.iter().any(|w| w == "crocodile"), "expected topic word from page: {:?}", topic.suggestions);

        // Do not suggest next word right after a period + space when starting a new sentence:
        assert!(analyze_word_at_caret("Il fait beau. ", 14, SpellLang::Fr).is_none());
    }

    #[test]
    fn orthography_scanner_and_languagetool_parser_work() {
        // 1. Offline homophones (`a/à`, `et/est`, `son/sont`, `ce/se`, `ou/où`), `-er/-é`, agreements & spelling:
        let sample = "Il à manger dans sont cahier. Nous parle de maizon.";
        let issues = scan_orthography_and_syntax(sample, SpellLang::Fr);
        let fixes: Vec<(&str, &str)> = issues.iter().map(|i| (i.original.as_str(), i.suggestions[0].as_str())).collect();
        assert!(fixes.contains(&("à", "a")), "expected 'à' -> 'a': {issues:?}");
        assert!(fixes.contains(&("manger", "mangé")), "expected 'manger' -> 'mangé': {issues:?}");
        assert!(fixes.contains(&("sont", "son")), "expected 'sont' -> 'son': {issues:?}");
        assert!(fixes.contains(&("parle", "parlons")), "expected 'parle' -> 'parlons': {issues:?}");
        assert!(fixes.iter().any(|&(o, s)| o == "maizon" && s == "maison"), "expected spelling fix: {issues:?}");

        let acc = scan_orthography_and_syntax("Les exercice et les chats", SpellLang::Fr);
        assert!(acc.iter().any(|i| i.original == "exercice" && i.suggestions.contains(&"exercices".to_string())), "plural fix: {acc:?}");

        // 2. Valid sentence produces zero false positives:
        let clean = scan_orthography_and_syntax("Je vais à la maison avec mes amis et mon cahier.", SpellLang::Fr);
        assert!(clean.is_empty(), "valid sentence should produce zero false alarms: {clean:?}");

        let clean_inflected =
            scan_orthography_and_syntax("Je prends mon livre et nous faisons des progrès. Mon fils connaît bien ses leçons.", SpellLang::Fr);
        assert!(clean_inflected.is_empty(), "valid inflected French sentence should produce zero false alarms: {clean_inflected:?}");

        // 2b. Full student phonetic/homophone sentence repair + pedagogical explanations:
        let noisy = "Je pren mon livre est nous féson dé progrè. mon fis connait bien ces lecons. ";
        let noisy_issues = scan_orthography_and_syntax(noisy, SpellLang::Fr);
        assert!(
            noisy_issues.iter().all(|iss| iss.message != "Mot inconnu ou mal orthographié."),
            "every issue must carry a pedagogical explanation: {noisy_issues:?}"
        );
        let mut repaired: Vec<char> = noisy.chars().collect();
        for iss in noisy_issues.iter().rev() {
            let rep = &iss.suggestions[0];
            repaired.splice(iss.char_range.0..iss.char_range.1, rep.chars());
        }
        let repaired: String = repaired.into_iter().collect();
        assert_eq!(repaired, "Je prends mon livre et nous faisons des progrès. Mon fils connaît bien ses leçons. ", "issues={noisy_issues:?}");

        // 3. LanguageTool JSON parser extracts offset/length/message/replacements cleanly:
        let json = r#"{"matches":[{"message":"Accord du verbe « parlons » attendu.","offset":5,"length":5,"replacements":[{"value":"parlons"},{"value":"parlions"}]}]}"#;
        let lt_issues = parse_languagetool_matches(json, "Nous parle bien.");
        assert_eq!(lt_issues.len(), 1);
        assert_eq!(lt_issues[0].original, "parle");
        assert_eq!(lt_issues[0].suggestions, vec!["parlons", "parlions"]);
    }
}
