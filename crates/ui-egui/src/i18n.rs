// Interface-language lookup. English UI wording is sourced from i18n/en.tsv; stable IDs are
// resolved by the generated catalog map and locale parsing/matching stays in craft-i18n.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use craft_i18n::{I18n, Lang};

include!(concat!(env!("OUT_DIR"), "/i18n_source_ids.rs"));

/// Follow the platform's preferred language until the user chooses one explicitly.
pub const AUTO: &str = "auto";

/// Languages shown by NoteCraft. Only French and English are maintained for now; the owner
/// decided on 2026-11-19 that the other catalogs stay on disk but are neither offered nor
/// loaded.
static EMBEDDED: &craft_i18n::Embedded = &[("en", "", include_str!("../../../i18n/en.tsv")), ("fr", "", include_str!("../../../i18n/fr.tsv"))];
static OFFERED: &[&str] = &["en", "fr"];
static PSEUDO_LOOKED_UP: LazyLock<Mutex<std::collections::BTreeSet<String>>> = LazyLock::new(|| Mutex::new(std::collections::BTreeSet::new()));
static AR_XB_RENDERED: LazyLock<Mutex<BTreeMap<String, &'static str>>> = LazyLock::new(|| Mutex::new(BTreeMap::new()));

static I18N: LazyLock<I18n> = LazyLock::new(|| {
    let (i18n, problems) = I18n::new(EMBEDDED, OFFERED);
    problems.iter().filter(|problem| !problem.starts_with("en: reserved")).for_each(|problem| log::warn!("interface catalogs: {problem}"));
    i18n
});

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageChoice {
    pub code: String,
    pub name: String,
}

/// Languages NoteCraft can actually display, with their names in their own language.
pub fn available_languages() -> Vec<LanguageChoice> {
    let mut languages: Vec<LanguageChoice> = I18N
        .languages()
        .into_iter()
        .map(|language| LanguageChoice { code: canonical_code(I18N.code(language)), name: I18N.name(language).to_string() })
        .collect();
    languages.sort_by_key(|language| match language.code.as_str() {
        "en" => 0,
        _ => 1,
    });
    languages
}

/// Code for the locale currently drawing this UI, resolving `auto` to its selected system locale.
pub fn current_code() -> String {
    canonical_code(I18N.code(CURRENT.get()))
}

fn canonical_code(code: &str) -> String {
    match code {
        "en-xa" => "en-XA".to_string(),
        "ar-xb" => "ar-XB".to_string(),
        _ => code.to_string(),
    }
}

/// Canonicalize a saved language code. Unknown or obsolete values return to automatic mode.
pub fn normalize_preference(preference: &str) -> String {
    if preference.eq_ignore_ascii_case(AUTO) {
        return AUTO.to_string();
    }
    I18N.from_code(preference).map_or_else(|| AUTO.to_string(), |language| I18N.code(language).to_string())
}

/// Resolve a preference against ordered system locale tags. This pure form keeps locale choice
/// deterministic in tests; the running app passes the platform's preferred tags.
pub fn resolve_preference(preference: &str, system_tags: &[&str]) -> String {
    let preference = normalize_preference(preference);
    if preference != AUTO {
        return preference;
    }
    let language =
        I18N.match_tags(system_tags.iter().copied().filter(|tag| !tag.eq_ignore_ascii_case("en-XA") && !tag.eq_ignore_ascii_case("ar-XB")));
    I18N.code(language).to_string()
}

/// Apply a saved preference for this UI thread. Automatic mode uses the shared runtime's
/// platform-aware system-language detection and falls back to English when no catalog matches.
pub fn set_current(preference: &str) {
    let preference = normalize_preference(preference);
    let language = match preference.as_str() {
        AUTO => I18N.system_lang(),
        _ => I18N.from_code(&preference).unwrap_or(Lang::EN),
    };
    CURRENT.set(language);
}

/// Translate one source string in the current language. Missing rows, including pseudo-catalog
/// rows, remain English so the pseudo locale can expose catalog coverage gaps.
pub fn t(source: &str) -> &str {
    let Some(id) = source_id(source) else {
        return source;
    };
    t_id(id)
}

/// Translate a stable UI catalog ID, falling back to the English value in `i18n/en.tsv`.
pub fn t_id(id: &str) -> &'static str {
    let language = CURRENT.get();
    if is_pseudo(language) {
        PSEUDO_LOOKED_UP.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned());
    }
    lookup_id(language, id)
}

fn lookup_id(language: Lang, id: &str) -> &'static str {
    let Some(english) = english_text(id) else {
        log::warn!("interface catalog has no English value for ID {id:?}");
        return "[missing UI translation]";
    };
    let translated = I18N.tid(language, id, english);
    if I18N.code(language) != "ar-xb" || translated == english {
        return translated;
    }
    let mut rendered = AR_XB_RENDERED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    rendered.entry(translated.to_owned()).or_insert_with(move || Box::leak(pseudo_render(language, translated).into_boxed_str()))
}

fn pseudo_render(language: Lang, text: &str) -> String {
    if I18N.code(language) == "ar-xb" { text.replace("[[RLI]]", "\u{2067}").replace("[[PDI]]", "\u{2069}") } else { text.to_owned() }
}

/// Source strings observed by the catalog runtime, for UI coverage audits.
pub fn pseudo_looked_up() -> std::collections::BTreeSet<String> {
    PSEUDO_LOOKED_UP.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn is_pseudo(language: Lang) -> bool {
    matches!(I18N.code(language), "en-xa" | "ar-xb")
}

/// Remove all ASCII pseudo delimiters while preserving source and dynamic wording.
pub fn pseudo_unmarked(text: &str) -> String {
    text.replace("$$$", "").replace("@@@", "").replace("\u{2067}", "").replace("\u{2069}", "").replace("~!~", "")
}

/// Keep runtime-provided text unchanged. In pseudo locales, only a catalog lookup may mark it;
/// wrapping arbitrary dynamic text here would conceal missing extraction.
pub fn pseudo_dynamic(text: &str) -> String {
    text.to_string()
}

/// Translate and format a user-facing message with named placeholders.
pub fn format_message(source: &str, args: &[(&str, &str)]) -> String {
    match source_id(source) {
        Some(id) => format_message_id(id, args),
        None => craft_i18n::fmt(source, args),
    }
}

/// Translate and format a message by stable catalog ID.
pub fn format_message_id(id: &str, args: &[(&str, &str)]) -> String {
    let language = CURRENT.get();
    if is_pseudo(language) {
        PSEUDO_LOOKED_UP.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned());
    }
    pseudo_render(language, &craft_i18n::fmt(t_id(id), args))
}

/// Format a count with the locale's plural rule, translating both the singular and plural forms.
pub fn format_count(one: &str, other: &str, count: u64) -> String {
    let language = CURRENT.get();
    let pair = format!("{one}|{other}");
    if let Some(id) = source_id(&pair) {
        return format_count_id(id, count);
    }
    I18N.trn(language, count, one, other)
}

/// Format a plural message by stable catalog ID. The English catalog stores its singular and
/// plural source forms separated by `|`; craft-i18n selects the locale's plural form.
pub fn format_count_id(id: &str, count: u64) -> String {
    let Some(pair) = english_text(id) else {
        log::warn!("interface catalog has no English plural value for ID {id:?}");
        return count.to_string();
    };
    let Some((one, other)) = pair.split_once('|') else {
        log::warn!("interface catalog plural ID {id:?} must contain one|other forms");
        return pair.to_string();
    };
    let language = CURRENT.get();
    if is_pseudo(language) {
        PSEUDO_LOOKED_UP.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned());
    }
    pseudo_render(language, &I18N.trn(language, count, one, other))
}

/// Format a plural template that also contains named placeholders after its count is selected.
pub fn format_count_message_id(id: &str, count: u64, args: &[(&str, &str)]) -> String {
    let Some(pair) = english_text(id) else {
        log::warn!("interface catalog has no English plural value for ID {id:?}");
        return id.to_string();
    };
    let Some((one, other)) = pair.split_once('|') else {
        log::warn!("interface catalog plural ID {id:?} must contain one|other forms");
        return pair.to_string();
    };
    let language = CURRENT.get();
    if is_pseudo(language) {
        PSEUDO_LOOKED_UP.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.to_owned());
    }
    let translated = I18N.trn(language, count, one, other);
    pseudo_render(language, &craft_i18n::fmt(&translated, args))
}

/// Translate and format a user-facing message in a specified language, falling back to English.
pub fn format_message_for(language: &str, source: &str, args: &[(&str, &str)]) -> String {
    let language = I18N.from_code(language).unwrap_or(Lang::EN);
    let Some(id) = source_id(source) else {
        return craft_i18n::fmt(source, args);
    };
    pseudo_render(language, &craft_i18n::fmt(lookup_id(language, id), args))
}

/// Translate a source string in a specified language, falling back to English.
pub fn translate_for(language: &str, source: &str) -> String {
    let language = I18N.from_code(language).unwrap_or(Lang::EN);
    source_id(source).map_or_else(|| source.to_string(), |id| lookup_id(language, id).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_french_and_english_are_offered() {
        let languages = available_languages();
        let codes: Vec<&str> = languages.iter().map(|language| language.code.as_str()).collect();
        assert_eq!(codes, ["en", "fr"], "the owner asked for French and English only (2026-11-19)");
        assert_eq!(languages[0].name, "English");
        assert_eq!(languages[1].name, "Français");
        assert!(I18N.check_all().is_empty(), "invalid embedded catalog rows: {:?}", I18N.check_all());
    }

    #[test]
    fn a_retired_language_falls_back_to_automatic() {
        assert_eq!(normalize_preference("es"), AUTO);
        assert_eq!(normalize_preference("de"), AUTO);
        assert_eq!(normalize_preference("en-XA"), AUTO);
        assert_eq!(normalize_preference("ar-XB"), AUTO);
        assert_eq!(normalize_preference("fr"), "fr");
        assert_eq!(normalize_preference("EN"), "en");
        assert_eq!(resolve_preference("es", &["fr-FR"]), "fr");
        assert_eq!(resolve_preference(AUTO, &["es-ES", "fr-FR"]), "fr");
        assert_eq!(resolve_preference(AUTO, &["ja-JP"]), "en");
    }

    #[test]
    fn french_translates_the_pilot_and_missing_strings_fall_back_to_english() {
        assert_eq!(translate_for("fr", "Interface language"), "Langue de l’interface");
        assert_eq!(translate_for("fr", "Notebook title"), "Notebook title");
    }

    #[test]
    fn french_covers_ui_strings_visible_in_the_localization_screenshots() {
        for (source, expected) in [
            ("Search all pages", "Rechercher dans toutes les pages"),
            ("Add page", "Ajouter une page"),
            ("A new page in this section", "Nouvelle page dans cette section"),
            ("Drop a file onto the page to add it", "Déposez un fichier sur la page pour l’ajouter"),
            ("Saved", "Enregistré"),
            ("Editing", "Modification en cours"),
            ("Tags", "Étiquettes"),
            ("NOTEBOOKS", "BLOCS-NOTES"),
            ("Ready", "Prêt"),
            ("Opened", "Page ouverte"),
            ("Default format", "Format par défaut"),
            ("New pages:", "Nouvelles pages :"),
        ] {
            assert_eq!(translate_for("fr", source), expected, "French catalog entry for {source}");
        }
        let french = I18N.from_code("fr").unwrap_or(Lang::EN);
        assert_eq!(I18N.trn(french, 2, "{n} notebook", "{n} notebooks"), "2 blocs-notes");
        assert_eq!(I18N.trn(french, 14, "{n} item", "{n} items"), "14 éléments");
    }

    #[test]
    fn user_facing_message_lookup_formats_translated_placeholders() {
        assert_eq!(format_message_for("fr", "Interface language: {language}", &[("language", "English")]), "Langue de l’interface : English");
        assert_eq!(
            format_message_for("fr", "Could not save the interface language: {error}", &[("error", "disk full")]),
            "Impossible d’enregistrer la langue de l’interface : disk full"
        );
    }

    #[test]
    fn counts_use_the_shared_locale_plural_rules() {
        CURRENT.set(Lang::EN);
        assert_eq!(format_count("{n} item", "{n} items", 1), "1 item");
        assert_eq!(format_count("{n} item", "{n} items", 2), "2 items");
    }

    #[test]
    fn saved_language_preferences_are_canonical_and_reject_unknown_languages() {
        assert_eq!(normalize_preference("FR"), "fr");
        assert_eq!(normalize_preference("EN"), "en");
        assert_eq!(normalize_preference("en"), "en");
        assert_eq!(normalize_preference("EN-xa"), AUTO);
        assert_eq!(normalize_preference("xx"), AUTO);
    }

    #[test]
    fn automatic_language_uses_the_first_supported_system_locale_or_english() {
        assert_eq!(resolve_preference(AUTO, &["fr-CA", "en-US"]), "fr");
        assert_eq!(resolve_preference(AUTO, &["de-DE"]), "en");
        assert_eq!(resolve_preference(AUTO, &["xx-YY"]), "en");
        assert_eq!(resolve_preference("en", &["fr-FR"]), "en");
    }
}
