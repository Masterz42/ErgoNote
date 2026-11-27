//! Vector stickers & school chemistry diagrams embedded as SVG bytes so they render
//! identically in colour on Linux, Windows, macOS, Android, and iOS without depending on
//! system emoji fonts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Smiley,
    Emoji,
    Labo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabGroup {
    Verrerie,
    Mesure,
    Securite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Printemps,
    Ete,
    Automne,
    Hiver,
}

impl Season {
    pub fn label(self) -> &'static str {
        match self {
            Self::Printemps => "Printemps",
            Self::Ete => "Été",
            Self::Automne => "Automne",
            Self::Hiver => "Hiver",
        }
    }
}

pub fn season_for_month_day(month: u32, day: u32) -> Season {
    let md = month.saturating_mul(100).saturating_add(day);
    if (320..=620).contains(&md) {
        Season::Printemps
    } else if (621..=921).contains(&md) {
        Season::Ete
    } else if (922..=1220).contains(&md) {
        Season::Automne
    } else {
        Season::Hiver
    }
}

pub fn current_season() -> Season {
    let secs = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let z = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = u64::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    season_for_month_day(u32::try_from(m).unwrap_or(1), u32::try_from(d).unwrap_or(1))
}

#[derive(Clone, Copy, Debug)]
pub struct Sticker {
    pub id: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub lab_group: Option<LabGroup>,
    pub width: f32,
    pub height: f32,
    pub svg: &'static str,
}

pub const SMILEYS: &[Sticker] = &[
    Sticker {
        id: "smiley-sourire",
        label: "Sourire",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.4" fill="#3B2B09"/><circle cx="42" cy="25" r="3.4" fill="#3B2B09"/><path d="M20 37 Q32 49 44 37" fill="none" stroke="#3B2B09" stroke-width="3.2" stroke-linecap="round"/><circle cx="16" cy="33" r="3.8" fill="#FF8C82" opacity="0.55"/><circle cx="48" cy="33" r="3.8" fill="#FF8C82" opacity="0.55"/></svg>"##,
    },
    Sticker {
        id: "smiley-rire",
        label: "Grand sourire",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M18 25 Q22 20 26 25" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M38 25 Q42 20 46 25" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M18 35 H46 Q44 50 32 50 Q20 50 18 35 Z" fill="#FFFFFF" stroke="#3B2B09" stroke-width="2.5" stroke-linejoin="round"/><path d="M23 44 Q32 49 41 44 Q38 49 32 49 Q26 49 23 44 Z" fill="#F05D5E"/></svg>"##,
    },
    Sticker {
        id: "smiley-clin-doeil",
        label: "Clin d'œil",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.5" fill="#3B2B09"/><path d="M37 25 Q42 21 47 26" fill="none" stroke="#3B2B09" stroke-width="3.2" stroke-linecap="round"/><path d="M20 38 Q33 48 45 36" fill="none" stroke="#3B2B09" stroke-width="3.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-cool",
        label: "Cool",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><rect x="12" y="19" width="18" height="11" rx="3" fill="#1F2937"/><rect x="34" y="19" width="18" height="11" rx="3" fill="#1F2937"/><line x1="29" y1="23" x2="35" y2="23" stroke="#1F2937" stroke-width="3"/><line x1="15" y1="22" x2="21" y2="28" stroke="#93C5FD" stroke-width="1.6" stroke-linecap="round"/><line x1="37" y1="22" x2="43" y2="28" stroke="#93C5FD" stroke-width="1.6" stroke-linecap="round"/><path d="M21 40 Q32 47 43 39" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-reflechi",
        label: "Réfléchi",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M17 19 L26 21" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><path d="M37 18 Q42 15 47 18" fill="none" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><circle cx="22" cy="26" r="3" fill="#3B2B09"/><circle cx="42" cy="25" r="3" fill="#3B2B09"/><path d="M24 39 Q32 37 40 40" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M18 52 L24 41 L29 44 L26 55 Z" fill="#F4B400" stroke="#A16207" stroke-width="2" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-surpris",
        label: "Surpris",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="24" r="3.6" fill="#3B2B09"/><circle cx="42" cy="24" r="3.6" fill="#3B2B09"/><path d="M17 17 Q22 14 27 17" fill="none" stroke="#3B2B09" stroke-width="2.2" stroke-linecap="round"/><path d="M37 17 Q42 14 47 17" fill="none" stroke="#3B2B09" stroke-width="2.2" stroke-linecap="round"/><ellipse cx="32" cy="42" rx="6" ry="7.5" fill="#7F1D1D" stroke="#3B2B09" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "smiley-triste",
        label: "Triste",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.3" fill="#3B2B09"/><circle cx="42" cy="25" r="3.3" fill="#3B2B09"/><path d="M21 44 Q32 35 43 44" fill="none" stroke="#3B2B09" stroke-width="3.2" stroke-linecap="round"/><path d="M46 31 Q51 37 46 41 Q41 37 46 31 Z" fill="#38BDF8" stroke="#0284C7" stroke-width="1.5"/></svg>"##,
    },
    Sticker {
        id: "smiley-fache",
        label: "Fâché",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FB923C" stroke="#C2410C" stroke-width="2.5"/><line x1="16" y1="19" x2="27" y2="24" stroke="#3B1A08" stroke-width="3" stroke-linecap="round"/><line x1="48" y1="19" x2="37" y2="24" stroke="#3B1A08" stroke-width="3" stroke-linecap="round"/><circle cx="22" cy="27" r="3.2" fill="#3B1A08"/><circle cx="42" cy="27" r="3.2" fill="#3B1A08"/><path d="M22 44 Q32 37 42 44" fill="none" stroke="#3B1A08" stroke-width="3.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-fete",
        label: "Fête",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="35" r="25" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><polygon points="22,6 42,15 26,22" fill="#EC4899" stroke="#9D174D" stroke-width="1.8" stroke-linejoin="round"/><circle cx="22" cy="11" r="2.2" fill="#60A5FA"/><circle cx="48" cy="16" r="2.2" fill="#34D399"/><circle cx="13" cy="20" r="2" fill="#F43F5E"/><path d="M19 32 Q23 28 27 32" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M37 32 Q41 28 45 32" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M22 42 Q32 51 42 42" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-etoiles",
        label: "Émerveillé",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><polygon points="21,18 23.4,22.8 28.6,23.5 24.8,27.2 25.7,32.3 21,29.9 16.3,32.3 17.2,27.2 13.4,23.5 18.6,22.8" fill="#2563EB" stroke="#1E3A8A" stroke-width="1.2"/><polygon points="43,18 45.4,22.8 50.6,23.5 46.8,27.2 47.7,32.3 43,29.9 38.3,32.3 39.2,27.2 35.4,23.5 40.6,22.8" fill="#2563EB" stroke="#1E3A8A" stroke-width="1.2"/><path d="M20 39 Q32 50 44 39" fill="#FFFFFF" stroke="#3B2B09" stroke-width="2.6" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-endormi",
        label: "Fatigué",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="34" r="26" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M17 30 Q22 34 27 30" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M37 30 Q42 34 47 30" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><ellipse cx="32" cy="44" rx="4.5" ry="4.5" fill="#7F1D1D"/><path d="M44 10 H53 L44 18 H53" fill="none" stroke="#2563EB" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-pouce",
        label: "Bravo !",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="8" y="30" width="12" height="24" rx="3" fill="#2563EB" stroke="#1E3A8A" stroke-width="2"/><path d="M20 32 L29 14 Q32 10 36 12 Q38 14 36 22 L34 28 H50 Q55 28 54 33 L51 49 Q50 54 44 54 H20 Z" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-coeur-yeux",
        label: "Amoureux",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M21 31 C15 26 13 22 16 18 C18 16 21 17 21 20 C21 17 24 16 26 18 C29 22 27 26 21 31 Z" fill="#EF4444"/><path d="M43 31 C37 26 35 22 38 18 C40 16 43 17 43 20 C43 17 46 16 48 18 C51 22 49 26 43 31 Z" fill="#EF4444"/><path d="M20 38 Q32 50 44 38" fill="#FFFFFF" stroke="#3B2B09" stroke-width="2.5" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-langue",
        label: "Taquin",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="24" r="3.4" fill="#3B2B09"/><path d="M37 25 Q42 20 47 25" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M20 36 H44" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M26 36 V46 A6 6 0 0 0 38 46 V36 Z" fill="#F43F5E" stroke="#9F1239" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "smiley-ange",
        label: "Ange",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="36" r="24" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><ellipse cx="32" cy="10" rx="18" ry="4.5" fill="none" stroke="#38BDF8" stroke-width="3.2"/><path d="M19 32 Q23 28 27 32" fill="none" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><path d="M37 32 Q41 28 45 32" fill="none" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><path d="M23 43 Q32 50 41 43" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-nerveux",
        label: "Gêné",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.4" fill="#3B2B09"/><circle cx="42" cy="25" r="3.4" fill="#3B2B09"/><path d="M19 42 Q26 38 32 42 Q38 46 45 42" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M51 12 Q57 20 51 25 Q45 20 51 12 Z" fill="#38BDF8" stroke="#0284C7" stroke-width="1.6"/></svg>"##,
    },
    Sticker {
        id: "smiley-inquiet",
        label: "Inquiet",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M16 20 Q21 17 26 19" fill="none" stroke="#3B2B09" stroke-width="2.4" stroke-linecap="round"/><path d="M38 19 Q43 17 48 20" fill="none" stroke="#3B2B09" stroke-width="2.4" stroke-linecap="round"/><circle cx="22" cy="27" r="3.3" fill="#3B2B09"/><circle cx="42" cy="27" r="3.3" fill="#3B2B09"/><path d="M22 44 Q32 39 42 44" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-malade",
        label: "Malade",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#A3E635" stroke="#4D7C0F" stroke-width="2.5"/><circle cx="22" cy="25" r="3.3" fill="#1F2937"/><circle cx="42" cy="25" r="3.3" fill="#1F2937"/><rect x="19" y="34" width="26" height="16" rx="5" fill="#F8FAFC" stroke="#475569" stroke-width="2"/><line x1="12" y1="36" x2="19" y2="39" stroke="#64748B" stroke-width="2"/><line x1="52" y1="36" x2="45" y2="39" stroke="#64748B" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "smiley-intello",
        label: "Intello",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="26" r="8" fill="#FFFFFF" opacity="0.55" stroke="#1E293B" stroke-width="2.8"/><circle cx="42" cy="26" r="8" fill="#FFFFFF" opacity="0.55" stroke="#1E293B" stroke-width="2.8"/><line x1="30" y1="26" x2="34" y2="26" stroke="#1E293B" stroke-width="2.8"/><circle cx="22" cy="26" r="2.6" fill="#1E293B"/><circle cx="42" cy="26" r="2.6" fill="#1E293B"/><path d="M22 41 Q32 48 42 41" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-siffle",
        label: "Siffle",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.4" fill="#3B2B09"/><circle cx="42" cy="25" r="3.4" fill="#3B2B09"/><circle cx="37" cy="41" r="4" fill="#3B2B09"/><circle cx="51" cy="35" r="3" fill="#2563EB"/><line x1="54" y1="35" x2="54" y2="24" stroke="#2563EB" stroke-width="2.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-chut",
        label: "Chut !",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="22" cy="25" r="3.4" fill="#3B2B09"/><circle cx="42" cy="25" r="3.4" fill="#3B2B09"/><line x1="24" y1="41" x2="40" y2="41" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><rect x="29" y="34" width="6" height="22" rx="3" fill="#FDE047" stroke="#A16207" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "smiley-explose",
        label: "Époustouflé",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M12 24 C8 12 22 6 32 10 C42 6 56 12 52 24 Z" fill="#F97316" stroke="#C2410C" stroke-width="2"/><path d="M10 30 H54 A24 24 0 0 1 10 30 Z" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><circle cx="23" cy="38" r="3" fill="#3B2B09"/><circle cx="41" cy="38" r="3" fill="#3B2B09"/><ellipse cx="32" cy="48" rx="4.5" ry="5" fill="#7F1D1D"/></svg>"##,
    },
    Sticker {
        id: "smiley-robot",
        label: "Robot",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="32" y1="6" x2="32" y2="16" stroke="#475569" stroke-width="3"/><circle cx="32" cy="6" r="3.5" fill="#EF4444"/><rect x="12" y="16" width="40" height="38" rx="8" fill="#CBD5E1" stroke="#334155" stroke-width="2.6"/><circle cx="23" cy="31" r="4.5" fill="#38BDF8" stroke="#0284C7" stroke-width="1.6"/><circle cx="41" cy="31" r="4.5" fill="#38BDF8" stroke="#0284C7" stroke-width="1.6"/><rect x="20" y="42" width="24" height="6" rx="2" fill="#1E293B"/><line x1="28" y1="42" x2="28" y2="48" stroke="#CBD5E1" stroke-width="1.5"/><line x1="36" y1="42" x2="36" y2="48" stroke="#CBD5E1" stroke-width="1.5"/></svg>"##,
    },
    Sticker {
        id: "smiley-bravo",
        label: "Applaudissements",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FEF08A" stroke="#CA8A04" stroke-width="2.4"/><line x1="16" y1="12" x2="21" y2="18" stroke="#F97316" stroke-width="2.6" stroke-linecap="round"/><line x1="32" y1="8" x2="32" y2="15" stroke="#F97316" stroke-width="2.6" stroke-linecap="round"/><line x1="48" y1="12" x2="43" y2="18" stroke="#F97316" stroke-width="2.6" stroke-linecap="round"/><path d="M18 46 L26 24 Q28 21 31 23 L35 41 Z" fill="#FACC15" stroke="#A16207" stroke-width="2.2" stroke-linejoin="round"/><path d="M27 48 L39 26 Q42 24 44 27 L38 49 Z" fill="#FDE047" stroke="#A16207" stroke-width="2.2" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-dort",
        label: "Gros dodo",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="34" r="26" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M16 31 Q21 35 26 31" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M36 31 Q41 35 46 31" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><circle cx="31" cy="44" r="4" fill="#7F1D1D"/><path d="M40 9 H50 L40 17 H50" fill="none" stroke="#1D4ED8" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/><path d="M49 20 H56 L49 26 H56" fill="none" stroke="#3B82F6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-sueur",
        label: "Soulagé",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M17 25 Q22 21 27 25" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M37 25 Q42 21 47 25" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M22 39 Q32 45 42 39" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M49 14 Q55 22 49 28 Q43 22 49 14 Z" fill="#38BDF8" stroke="#0284C7" stroke-width="1.6"/></svg>"##,
    },
    Sticker {
        id: "smiley-bisou",
        label: "Bisou",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><circle cx="21" cy="24" r="3.4" fill="#3B2B09"/><path d="M36 25 Q41 21 46 25" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/><path d="M28 36 Q33 38 29 41 Q33 44 28 46" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><path d="M47 46 C42 41 40 37 43 34 C45 32 47 33 47 36 C47 33 49 32 51 34 C54 37 52 41 47 46 Z" fill="#EF4444"/></svg>"##,
    },
    Sticker {
        id: "smiley-clown",
        label: "Rigolo",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="14" cy="20" r="8" fill="#3B82F6"/><circle cx="50" cy="20" r="8" fill="#EF4444"/><circle cx="32" cy="34" r="25" fill="#FEF9C3" stroke="#CA8A04" stroke-width="2.4"/><circle cx="22" cy="27" r="3.2" fill="#1E293B"/><circle cx="42" cy="27" r="3.2" fill="#1E293B"/><circle cx="32" cy="35" r="5.5" fill="#EF4444" stroke="#991B1B" stroke-width="1.5"/><path d="M20 44 Q32 53 44 44" fill="none" stroke="#DC2626" stroke-width="3.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-zen",
        label: "Zen",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="28" fill="#FFD93B" stroke="#C98A00" stroke-width="2.5"/><path d="M17 26 Q22 30 27 26" fill="none" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><path d="M37 26 Q42 30 47 26" fill="none" stroke="#3B2B09" stroke-width="2.6" stroke-linecap="round"/><path d="M23 40 Q32 45 41 40" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><circle cx="16" cy="34" r="3.8" fill="#FCA5A5" opacity="0.65"/><circle cx="48" cy="34" r="3.8" fill="#FCA5A5" opacity="0.65"/></svg>"##,
    },
    Sticker {
        id: "smiley-cowboy",
        label: "Aventure",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="36" r="24" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><ellipse cx="32" cy="20" rx="28" ry="5" fill="#92400E" stroke="#451A03" stroke-width="2"/><path d="M19 19 L23 8 Q32 11 41 8 L45 19 Z" fill="#B45309" stroke="#451A03" stroke-width="2" stroke-linejoin="round"/><circle cx="23" cy="34" r="3" fill="#3B2B09"/><circle cx="41" cy="34" r="3" fill="#3B2B09"/><path d="M22 43 Q32 51 42 43" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-magie",
        label: "Magicien",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="37" r="23" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><ellipse cx="32" cy="22" rx="25" ry="4" fill="#312E81"/><polygon points="21,21 25,6 39,6 43,21" fill="#4338CA" stroke="#1E1B4B" stroke-width="2"/><polygon points="32,9 33.8,13.2 38,14 34.8,16.8 35.6,21 32,18.8 28.4,21 29.2,16.8 26,14 30.2,13.2" fill="#FDE047"/><circle cx="23" cy="36" r="3" fill="#3B2B09"/><circle cx="41" cy="36" r="3" fill="#3B2B09"/><path d="M23 45 Q32 51 41 45" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-diplome",
        label: "Diplômé",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="37" r="23" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><polygon points="32,6 56,16 32,26 8,16" fill="#1E293B" stroke="#0F172A" stroke-width="1.8"/><rect x="20" y="21" width="24" height="6" rx="2" fill="#334155"/><line x1="51" y1="18" x2="51" y2="30" stroke="#F59E0B" stroke-width="2.5"/><circle cx="23" cy="36" r="3" fill="#3B2B09"/><circle cx="41" cy="36" r="3" fill="#3B2B09"/><path d="M23 45 Q32 51 41 45" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-fleur",
        label: "Printemps",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="34" r="25" fill="#FFD93B" stroke="#C98A04" stroke-width="2.4"/><circle cx="48" cy="14" r="4.5" fill="#F472B6"/><circle cx="54" cy="19" r="4.5" fill="#F472B6"/><circle cx="48" cy="24" r="4.5" fill="#F472B6"/><circle cx="42" cy="19" r="4.5" fill="#F472B6"/><circle cx="48" cy="19" r="3.5" fill="#FDE047"/><circle cx="23" cy="32" r="3" fill="#3B2B09"/><circle cx="41" cy="32" r="3" fill="#3B2B09"/><path d="M22 41 Q32 49 42 41" fill="none" stroke="#3B2B09" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-plage",
        label: "Vacances",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="35" r="24" fill="#FACC15" stroke="#B45309" stroke-width="2.4"/><ellipse cx="32" cy="16" rx="26" ry="5" fill="#FDE68A" stroke="#CA8A04" stroke-width="2"/><path d="M20 15 Q32 6 44 15 Z" fill="#F59E0B" stroke="#B45309" stroke-width="1.8"/><rect x="15" y="26" width="14" height="8" rx="2.5" fill="#0F172A"/><rect x="35" y="26" width="14" height="8" rx="2.5" fill="#0F172A"/><line x1="29" y1="29" x2="35" y2="29" stroke="#0F172A" stroke-width="2.6"/><path d="M22 43 Q32 50 42 43" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "smiley-echarpe",
        label: "Automne",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="30" r="24" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><circle cx="23" cy="27" r="3.2" fill="#3B2B09"/><circle cx="41" cy="27" r="3.2" fill="#3B2B09"/><path d="M23 36 Q32 42 41 36" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/><rect x="13" y="43" width="38" height="10" rx="5" fill="#EA580C" stroke="#9A3412" stroke-width="2"/><rect x="35" y="48" width="10" height="13" rx="3" fill="#F97316" stroke="#9A3412" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "smiley-bonnet-hiver",
        label: "Hiver",
        category: Category::Smiley,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="37" r="23" fill="#FFD93B" stroke="#C98A00" stroke-width="2.4"/><circle cx="32" cy="8" r="5" fill="#F8FAFC" stroke="#94A3B8" stroke-width="1.6"/><path d="M12 24 C12 13 52 13 52 24 Z" fill="#EF4444" stroke="#991B1B" stroke-width="2"/><rect x="10" y="22" width="44" height="7" rx="3.5" fill="#F8FAFC" stroke="#94A3B8" stroke-width="1.8"/><circle cx="23" cy="37" r="3" fill="#3B2B09"/><circle cx="41" cy="37" r="3" fill="#3B2B09"/><path d="M23 46 Q32 51 41 46" fill="none" stroke="#3B2B09" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
];

pub const EMOJIS: &[Sticker] = &[
    Sticker {
        id: "emoji-etoile",
        label: "Étoile",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="32,6 39.5,22 57,24.5 44.2,36.8 47.4,54.2 32,45.8 16.6,54.2 19.8,36.8 7,24.5 24.5,22" fill="#FACC15" stroke="#B45309" stroke-width="2.6" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-ampoule",
        label: "Idée",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32 10 C21 10 15 18 15 27 C15 34 20 39 23 44 H41 C44 39 49 34 49 27 C49 18 43 10 32 10 Z" fill="#FDE047" stroke="#CA8A04" stroke-width="2.4"/><rect x="23" y="44" width="18" height="6" rx="2" fill="#CBD5E1" stroke="#475569" stroke-width="2"/><rect x="25" y="50" width="14" height="5" rx="2" fill="#94A3B8" stroke="#475569" stroke-width="1.8"/><line x1="32" y1="3" x2="32" y2="7" stroke="#EAB308" stroke-width="2.5" stroke-linecap="round"/><line x1="10" y1="14" x2="14" y2="17" stroke="#EAB308" stroke-width="2.5" stroke-linecap="round"/><line x1="54" y1="14" x2="50" y2="17" stroke="#EAB308" stroke-width="2.5" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-valide",
        label: "Validé",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" fill="#22C55E" stroke="#15803D" stroke-width="2.5"/><polyline points="19,33 28,42 46,22" fill="none" stroke="#FFFFFF" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-croix",
        label: "Erreur",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" fill="#EF4444" stroke="#B91C1C" stroke-width="2.5"/><line x1="21" y1="21" x2="43" y2="43" stroke="#FFFFFF" stroke-width="5" stroke-linecap="round"/><line x1="43" y1="21" x2="21" y2="43" stroke="#FFFFFF" stroke-width="5" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-attention",
        label: "Attention",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="32,7 58,54 6,54" fill="#FACC15" stroke="#B45309" stroke-width="3" stroke-linejoin="round"/><line x1="32" y1="22" x2="32" y2="39" stroke="#1E293B" stroke-width="4.8" stroke-linecap="round"/><circle cx="32" cy="47" r="2.8" fill="#1E293B"/></svg>"##,
    },
    Sticker {
        id: "emoji-cible",
        label: "Objectif",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" fill="#EF4444" stroke="#991B1B" stroke-width="2"/><circle cx="32" cy="32" r="18" fill="#FFFFFF"/><circle cx="32" cy="32" r="10" fill="#EF4444"/><circle cx="32" cy="32" r="4" fill="#FACC15"/><line x1="54" y1="10" x2="34" y2="30" stroke="#1E293B" stroke-width="3" stroke-linecap="round"/><polygon points="50,6 58,6 54,14" fill="#3B82F6"/></svg>"##,
    },
    Sticker {
        id: "emoji-trophee",
        label: "Trophée",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M18 12 H46 V26 C46 35 39 42 32 42 C25 42 18 35 18 26 Z" fill="#FBBF24" stroke="#B45309" stroke-width="2.4"/><path d="M18 16 H10 C10 25 14 29 19 29" fill="none" stroke="#B45309" stroke-width="2.8"/><path d="M46 16 H54 C54 25 50 29 45 29" fill="none" stroke="#B45309" stroke-width="2.8"/><rect x="29" y="42" width="6" height="8" fill="#D97706"/><rect x="20" y="50" width="24" height="6" rx="2" fill="#92400E"/></svg>"##,
    },
    Sticker {
        id: "emoji-coeur",
        label: "Cœur",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32 54 C14 42 7 32 7 21 C7 13 13 8 20 8 C26 8 30 12 32 16 C34 12 38 8 44 8 C51 8 57 13 57 21 C57 32 50 42 32 54 Z" fill="#F43F5E" stroke="#BE123C" stroke-width="2.4"/></svg>"##,
    },
    Sticker {
        id: "emoji-epingle",
        label: "Épingle",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="25" y1="39" x2="12" y2="54" stroke="#475569" stroke-width="3.5" stroke-linecap="round"/><path d="M22 36 L34 42 L44 30 L52 24 L40 12 L34 20 L22 30 Z" fill="#EF4444" stroke="#991B1B" stroke-width="2.2" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-loupe",
        label: "Observation",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="27" cy="27" r="17" fill="#DBEAFE" stroke="#1E3A8A" stroke-width="3.5"/><path d="M19 20 Q23 15 29 17" fill="none" stroke="#FFFFFF" stroke-width="2.8" stroke-linecap="round"/><line x1="39" y1="39" x2="55" y2="55" stroke="#78350F" stroke-width="6" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-livre",
        label: "Cours",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M8 14 Q20 10 32 16 Q44 10 56 14 V50 Q44 46 32 52 Q20 46 8 50 Z" fill="#EFF6FF" stroke="#1D4ED8" stroke-width="2.6" stroke-linejoin="round"/><line x1="32" y1="16" x2="32" y2="52" stroke="#1D4ED8" stroke-width="2.2"/><line x1="14" y1="24" x2="26" y2="24" stroke="#64748B" stroke-width="2"/><line x1="14" y1="32" x2="26" y2="32" stroke="#64748B" stroke-width="2"/><line x1="38" y1="24" x2="50" y2="24" stroke="#64748B" stroke-width="2"/><line x1="38" y1="32" x2="50" y2="32" stroke="#64748B" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-flamme",
        label: "Important",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M33 6 C38 16 49 24 49 38 C49 49 41 57 32 57 C21 57 15 49 15 39 C15 29 22 22 25 18 C25 24 29 26 30 22 C30 16 31 11 33 6 Z" fill="#F97316" stroke="#C2410C" stroke-width="2.2"/><path d="M32 29 C36 35 40 39 40 45 C40 50 36 53 32 53 C27 53 24 50 24 45 C24 39 29 35 32 29 Z" fill="#FDE047"/></svg>"##,
    },
    Sticker {
        id: "emoji-fusee",
        label: "Fusée",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32 6 C42 16 44 30 40 44 H24 C20 30 22 16 32 6 Z" fill="#F8FAFC" stroke="#1E293B" stroke-width="2.4"/><path d="M24 34 L13 44 L22 46 Z" fill="#EF4444" stroke="#991B1B" stroke-width="1.8"/><path d="M40 34 L51 44 L42 46 Z" fill="#EF4444" stroke="#991B1B" stroke-width="1.8"/><circle cx="32" cy="24" r="5.5" fill="#38BDF8" stroke="#1E3A8A" stroke-width="2"/><polygon points="27,44 37,44 32,58" fill="#F97316"/></svg>"##,
    },
    Sticker {
        id: "emoji-soleil",
        label: "Soleil",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="14" fill="#FACC15" stroke="#D97706" stroke-width="2.4"/><line x1="32" y1="6" x2="32" y2="13" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="32" y1="51" x2="32" y2="58" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="6" y1="32" x2="13" y2="32" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="51" y1="32" x2="58" y2="32" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="13" y1="13" x2="18" y2="18" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="46" y1="46" x2="51" y2="51" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="51" y1="13" x2="46" y2="18" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/><line x1="18" y1="46" x2="13" y2="51" stroke="#F59E0B" stroke-width="3.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-lune",
        label: "Lune",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M40 10 C25 10 14 21 14 35 C14 48 25 56 38 56 C46 56 52 52 55 46 C42 49 32 40 32 27 C32 20 35 14 40 10 Z" fill="#FDE047" stroke="#B45309" stroke-width="2.4" stroke-linejoin="round"/><circle cx="47" cy="18" r="2.4" fill="#FACC15"/><circle cx="53" cy="27" r="1.8" fill="#FACC15"/></svg>"##,
    },
    Sticker {
        id: "emoji-eclair",
        label: "Éclair / Énergie",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="36,6 15,35 30,35 24,58 49,28 34,28" fill="#FACC15" stroke="#B45309" stroke-width="2.5" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-planete",
        label: "Terre",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="25" fill="#3B82F6" stroke="#1E3A8A" stroke-width="2.4"/><path d="M18 18 Q28 22 25 32 Q22 40 28 46 Q16 44 13 31 Z" fill="#22C55E"/><path d="M38 14 Q48 20 46 31 Q40 36 44 46 Q53 40 53 28 Q50 18 38 14 Z" fill="#22C55E"/></svg>"##,
    },
    Sticker {
        id: "emoji-plante",
        label: "Plante / SVT",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="32" y1="54" x2="32" y2="26" stroke="#15803D" stroke-width="3.6" stroke-linecap="round"/><path d="M32 34 C16 34 11 22 14 14 C23 14 32 22 32 34 Z" fill="#22C55E" stroke="#15803D" stroke-width="2"/><path d="M32 28 C48 28 53 16 50 8 C41 8 32 16 32 28 Z" fill="#4ADE80" stroke="#15803D" stroke-width="2"/><rect x="18" y="52" width="28" height="5" rx="2.5" fill="#92400E"/></svg>"##,
    },
    Sticker {
        id: "emoji-musique",
        label: "Musique",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M24 46 V16 L50 10 V40" fill="none" stroke="#7C3AED" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/><line x1="24" y1="24" x2="50" y2="18" stroke="#7C3AED" stroke-width="3.5"/><circle cx="18" cy="46" r="7" fill="#A855F7" stroke="#5B21B6" stroke-width="2"/><circle cx="44" cy="40" r="7" fill="#A855F7" stroke="#5B21B6" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-palette",
        label: "Arts",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32 8 C16 8 8 19 8 32 C8 46 18 54 30 54 C36 54 39 50 38 45 C37 41 40 39 45 39 C52 39 56 34 56 26 C56 15 46 8 32 8 Z" fill="#FDE68A" stroke="#B45309" stroke-width="2.4"/><circle cx="20" cy="24" r="4" fill="#EF4444"/><circle cx="32" cy="18" r="4" fill="#3B82F6"/><circle cx="44" cy="23" r="4" fill="#22C55E"/><circle cx="19" cy="36" r="4" fill="#A855F7"/></svg>"##,
    },
    Sticker {
        id: "emoji-horloge",
        label: "Temps",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="32" r="25" fill="#F8FAFC" stroke="#2563EB" stroke-width="3.2"/><circle cx="32" cy="32" r="2.8" fill="#1E293B"/><line x1="32" y1="32" x2="32" y2="16" stroke="#1E293B" stroke-width="3.2" stroke-linecap="round"/><line x1="32" y1="32" x2="44" y2="38" stroke="#EF4444" stroke-width="2.8" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-medaille",
        label: "Médaille",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="20,6 30,6 34,26 24,26" fill="#2563EB"/><polygon points="44,6 34,6 30,26 40,26" fill="#EF4444"/><circle cx="32" cy="40" r="16" fill="#FACC15" stroke="#B45309" stroke-width="2.5"/><polygon points="32,30 34.8,36 41.2,36.8 36.5,41.3 37.7,47.6 32,44.5 26.3,47.6 27.5,41.3 22.8,36.8 29.2,36" fill="#FEF08A"/></svg>"##,
    },
    Sticker {
        id: "emoji-crayon",
        label: "Écriture",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="14,50 18,36 42,12 52,22 28,46" fill="#FACC15" stroke="#B45309" stroke-width="2.2" stroke-linejoin="round"/><polygon points="42,12 48,6 58,16 52,22" fill="#F472B6" stroke="#9D174D" stroke-width="2"/><polygon points="14,50 18,36 28,46" fill="#FDE68A" stroke="#92400E" stroke-width="1.8"/><polygon points="14,50 16,44 20,48" fill="#1E293B"/></svg>"##,
    },
    Sticker {
        id: "emoji-drapeau",
        label: "Étape",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="16" y1="8" x2="16" y2="56" stroke="#334155" stroke-width="4" stroke-linecap="round"/><path d="M18 10 Q32 6 40 12 Q48 16 54 12 V32 Q46 36 38 31 Q30 26 18 30 Z" fill="#22C55E" stroke="#15803D" stroke-width="2.2" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-fleur-cerisier",
        label: "Fleur",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="15" r="10" fill="#F9A8D4" stroke="#DB2777" stroke-width="2"/><circle cx="48" cy="27" r="10" fill="#F9A8D4" stroke="#DB2777" stroke-width="2"/><circle cx="42" cy="46" r="10" fill="#F9A8D4" stroke="#DB2777" stroke-width="2"/><circle cx="22" cy="46" r="10" fill="#F9A8D4" stroke="#DB2777" stroke-width="2"/><circle cx="16" cy="27" r="10" fill="#F9A8D4" stroke="#DB2777" stroke-width="2"/><circle cx="32" cy="32" r="8" fill="#FDE047" stroke="#CA8A04" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-papillon",
        label: "Papillon",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><ellipse cx="20" cy="24" rx="13" ry="11" fill="#38BDF8" stroke="#0284C7" stroke-width="2"/><ellipse cx="44" cy="24" rx="13" ry="11" fill="#38BDF8" stroke="#0284C7" stroke-width="2"/><ellipse cx="21" cy="42" rx="9" ry="8" fill="#A855F7" stroke="#6B21A8" stroke-width="2"/><ellipse cx="43" cy="42" rx="9" ry="8" fill="#A855F7" stroke="#6B21A8" stroke-width="2"/><rect x="30" y="16" width="4" height="34" rx="2" fill="#1E293B"/></svg>"##,
    },
    Sticker {
        id: "emoji-arc-en-ciel",
        label: "Arc-en-ciel",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M8 46 A24 24 0 0 1 56 46" fill="none" stroke="#EF4444" stroke-width="5"/><path d="M13 46 A19 19 0 0 1 51 46" fill="none" stroke="#FACC15" stroke-width="5"/><path d="M18 46 A14 14 0 0 1 46 46" fill="none" stroke="#22C55E" stroke-width="5"/><path d="M23 46 A9 9 0 0 1 41 46" fill="none" stroke="#3B82F6" stroke-width="5"/></svg>"##,
    },
    Sticker {
        id: "emoji-abeille",
        label: "Abeille",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><ellipse cx="26" cy="20" rx="8" ry="11" fill="#E0F2FE" stroke="#0284C7" stroke-width="2"/><ellipse cx="38" cy="20" rx="8" ry="11" fill="#E0F2FE" stroke="#0284C7" stroke-width="2"/><ellipse cx="32" cy="39" rx="18" ry="12" fill="#FACC15" stroke="#1E293B" stroke-width="2.4"/><line x1="25" y1="28" x2="25" y2="50" stroke="#1E293B" stroke-width="4"/><line x1="35" y1="27" x2="35" y2="51" stroke="#1E293B" stroke-width="4"/><circle cx="19" cy="36" r="2" fill="#1E293B"/></svg>"##,
    },
    Sticker {
        id: "emoji-glace",
        label: "Glace",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="25" cy="22" r="10" fill="#F472B6" stroke="#BE185D" stroke-width="2"/><circle cx="39" cy="22" r="10" fill="#A7F3D0" stroke="#059669" stroke-width="2"/><circle cx="32" cy="14" r="10" fill="#FDE68A" stroke="#D97706" stroke-width="2"/><polygon points="20,28 44,28 32,58" fill="#F59E0B" stroke="#92400E" stroke-width="2.2" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-palmier",
        label: "Palmier",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M30 56 Q34 38 30 22" fill="none" stroke="#92400E" stroke-width="6" stroke-linecap="round"/><path d="M30 22 C16 14 10 20 8 28 C18 24 24 24 30 22 Z" fill="#22C55E" stroke="#15803D" stroke-width="1.8"/><path d="M30 22 C44 14 52 20 56 28 C44 24 36 24 30 22 Z" fill="#16A34A" stroke="#15803D" stroke-width="1.8"/><path d="M30 20 C24 8 36 6 44 14 C36 14 32 16 30 20 Z" fill="#4ADE80" stroke="#15803D" stroke-width="1.8"/></svg>"##,
    },
    Sticker {
        id: "emoji-parasol",
        label: "Parasol",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="32" y1="24" x2="32" y2="56" stroke="#78350F" stroke-width="3.5" stroke-linecap="round"/><path d="M8 28 Q32 4 56 28 Z" fill="#EF4444" stroke="#991B1B" stroke-width="2.2"/><path d="M20 28 Q32 4 44 28 Z" fill="#FFFFFF"/><rect x="12" y="54" width="40" height="4" rx="2" fill="#FDE047"/></svg>"##,
    },
    Sticker {
        id: "emoji-pasteque",
        label: "Pastèque",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M8 24 A24 24 0 0 0 56 24 Z" fill="#16A34A" stroke="#14532D" stroke-width="2"/><path d="M13 24 A19 19 0 0 0 51 24 Z" fill="#F43F5E"/><circle cx="25" cy="30" r="1.8" fill="#1E293B"/><circle cx="32" cy="35" r="1.8" fill="#1E293B"/><circle cx="39" cy="30" r="1.8" fill="#1E293B"/></svg>"##,
    },
    Sticker {
        id: "emoji-feuille-automne",
        label: "Feuille",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="32,6 38,20 54,18 44,30 56,38 40,42 36,50 32,45 28,50 24,42 8,38 20,30 10,18 26,20" fill="#EA580C" stroke="#9A3412" stroke-width="2" stroke-linejoin="round"/><line x1="32" y1="45" x2="32" y2="58" stroke="#7C2D12" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-citrouille",
        label: "Citrouille",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="29" y="10" width="6" height="10" rx="2" fill="#15803D"/><ellipse cx="20" cy="36" rx="11" ry="18" fill="#EA580C" stroke="#9A3412" stroke-width="2"/><ellipse cx="44" cy="36" rx="11" ry="18" fill="#EA580C" stroke="#9A3412" stroke-width="2"/><ellipse cx="32" cy="36" rx="11" ry="19" fill="#F97316" stroke="#9A3412" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-champignon",
        label: "Champignon",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="24" y="32" width="16" height="24" rx="5" fill="#FEF3C7" stroke="#B45309" stroke-width="2.2"/><path d="M10 34 C10 14 54 14 54 34 Z" fill="#DC2626" stroke="#991B1B" stroke-width="2.2"/><circle cx="24" cy="24" r="3.5" fill="#FFFFFF"/><circle cx="40" cy="22" r="3" fill="#FFFFFF"/><circle cx="33" cy="17" r="2.6" fill="#FFFFFF"/></svg>"##,
    },
    Sticker {
        id: "emoji-gland",
        label: "Gland",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="32" y1="8" x2="32" y2="16" stroke="#78350F" stroke-width="3.5" stroke-linecap="round"/><ellipse cx="32" cy="39" rx="14" ry="17" fill="#D97706" stroke="#78350F" stroke-width="2.2"/><path d="M14 27 C14 14 50 14 50 27 Z" fill="#78350F" stroke="#451A03" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-parapluie",
        label: "Pluie",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32 30 V50 A5 5 0 0 0 42 50" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round"/><path d="M10 30 C10 12 54 12 54 30 Z" fill="#3B82F6" stroke="#1D4ED8" stroke-width="2.2"/><circle cx="16" cy="44" r="2.2" fill="#38BDF8"/><circle cx="50" cy="42" r="2.2" fill="#38BDF8"/></svg>"##,
    },
    Sticker {
        id: "emoji-flocon",
        label: "Flocon",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><line x1="32" y1="6" x2="32" y2="58" stroke="#38BDF8" stroke-width="3.5" stroke-linecap="round"/><line x1="10" y1="19" x2="54" y2="45" stroke="#38BDF8" stroke-width="3.5" stroke-linecap="round"/><line x1="54" y1="19" x2="10" y2="45" stroke="#38BDF8" stroke-width="3.5" stroke-linecap="round"/><circle cx="32" cy="32" r="5" fill="#E0F2FE" stroke="#0284C7" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "emoji-bonhomme-neige",
        label: "Bonhomme",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="32" cy="43" r="14" fill="#F8FAFC" stroke="#64748B" stroke-width="2.2"/><circle cx="32" cy="23" r="10" fill="#F8FAFC" stroke="#64748B" stroke-width="2.2"/><rect x="20" y="9" width="24" height="4" fill="#1E293B"/><rect x="24" y="4" width="16" height="7" fill="#1E293B"/><circle cx="28" cy="21" r="1.8" fill="#1E293B"/><circle cx="36" cy="21" r="1.8" fill="#1E293B"/><polygon points="32,23 41,25 32,26" fill="#F97316"/></svg>"##,
    },
    Sticker {
        id: "emoji-sapin",
        label: "Sapin",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="28" y="48" width="8" height="10" fill="#78350F"/><polygon points="32,16 52,48 12,48" fill="#15803D" stroke="#14532D" stroke-width="2" stroke-linejoin="round"/><polygon points="32,6 46,28 18,28" fill="#22C55E" stroke="#14532D" stroke-width="2" stroke-linejoin="round"/><circle cx="28" cy="36" r="2.8" fill="#EF4444"/><circle cx="38" cy="41" r="2.8" fill="#FACC15"/></svg>"##,
    },
    Sticker {
        id: "emoji-cadeau",
        label: "Cadeau",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="12" y="24" width="40" height="32" rx="3" fill="#EF4444" stroke="#991B1B" stroke-width="2.2"/><rect x="9" y="18" width="46" height="8" rx="2" fill="#F87171" stroke="#991B1B" stroke-width="2"/><rect x="28" y="18" width="8" height="38" fill="#FACC15"/><path d="M32 18 C22 10 18 14 24 18 M32 18 C42 10 46 14 40 18" fill="none" stroke="#FACC15" stroke-width="3" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "emoji-tasse-chaude",
        label: "Chocolat chaud",
        category: Category::Emoji,
        lab_group: None,
        width: 52.0,
        height: 52.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M14 26 H44 V44 A10 10 0 0 1 34 54 H24 A10 10 0 0 1 14 44 Z" fill="#FFFFFF" stroke="#78350F" stroke-width="2.4"/><path d="M44 30 H50 A5 5 0 0 1 50 42 H44" fill="none" stroke="#78350F" stroke-width="2.6"/><ellipse cx="29" cy="26" rx="14" ry="3.5" fill="#92400E"/><path d="M22 18 Q25 12 22 8" fill="none" stroke="#94A3B8" stroke-width="2.2" stroke-linecap="round"/><path d="M34 18 Q37 12 34 8" fill="none" stroke="#94A3B8" stroke-width="2.2" stroke-linecap="round"/></svg>"##,
    },
];

pub const LABO: &[Sticker] = &[
    Sticker {
        id: "labo-becher",
        label: "Bécher",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 110.0,
        height: 125.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M26 54 H74 V102 Q74 106 70 106 H30 Q26 106 26 102 Z" fill="#60A5FA" opacity="0.45"/><path d="M18 18 L24 24 V102 Q24 108 30 108 H70 Q76 108 76 102 V24 L80 18" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/><line x1="24" y1="22" x2="76" y2="22" stroke="#1E293B" stroke-width="2"/><ellipse cx="50" cy="54" rx="24" ry="3.5" fill="#93C5FD" stroke="#2563EB" stroke-width="1.5"/><line x1="34" y1="40" x2="46" y2="40" stroke="#1E293B" stroke-width="2"/><line x1="34" y1="54" x2="43" y2="54" stroke="#1E293B" stroke-width="2"/><line x1="34" y1="68" x2="46" y2="68" stroke="#1E293B" stroke-width="2"/><line x1="34" y1="82" x2="43" y2="82" stroke="#1E293B" stroke-width="2"/><line x1="34" y1="96" x2="46" y2="96" stroke="#1E293B" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "labo-erlenmeyer",
        label: "Erlenmeyer",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 110.0,
        height: 125.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><polygon points="35,70 65,70 80,100 76,106 24,106 20,100" fill="#2DD4BF" opacity="0.5"/><path d="M38 14 H62 M40 16 V42 L18 98 Q16 108 26 108 H74 Q84 108 82 98 L60 42 V16" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/><line x1="35" y1="70" x2="65" y2="70" stroke="#0F766E" stroke-width="2"/><line x1="36" y1="78" x2="45" y2="78" stroke="#1E293B" stroke-width="1.8"/><line x1="31" y1="90" x2="44" y2="90" stroke="#1E293B" stroke-width="1.8"/></svg>"##,
    },
    Sticker {
        id: "labo-eprouvette",
        label: "Éprouvette",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 95.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><rect x="38" y="48" width="24" height="54" fill="#38BDF8" opacity="0.45"/><path d="M34 14 L38 20 V102 H62 V20 L66 14" fill="none" stroke="#1E293B" stroke-width="2.8" stroke-linejoin="round"/><rect x="24" y="102" width="52" height="8" rx="3" fill="#CBD5E1" stroke="#1E293B" stroke-width="2.4"/><path d="M38 48 Q50 53 62 48" fill="none" stroke="#0284C7" stroke-width="2"/><line x1="42" y1="30" x2="50" y2="30" stroke="#1E293B" stroke-width="1.8"/><line x1="42" y1="42" x2="52" y2="42" stroke="#1E293B" stroke-width="1.8"/><line x1="42" y1="54" x2="50" y2="54" stroke="#1E293B" stroke-width="1.8"/><line x1="42" y1="66" x2="52" y2="66" stroke="#1E293B" stroke-width="1.8"/><line x1="42" y1="78" x2="50" y2="78" stroke="#1E293B" stroke-width="1.8"/><line x1="42" y1="90" x2="52" y2="90" stroke="#1E293B" stroke-width="1.8"/></svg>"##,
    },
    Sticker {
        id: "labo-tube-essai",
        label: "Tube à essai",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 90.0,
        height: 125.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M38 56 H62 V92 A12 12 0 0 1 38 92 Z" fill="#A855F7" opacity="0.45"/><path d="M34 16 H66 M38 16 V92 A12 12 0 0 0 62 92 V16" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round"/><line x1="38" y1="56" x2="62" y2="56" stroke="#7E22CE" stroke-width="2"/><circle cx="47" cy="76" r="2.5" fill="#FFFFFF"/><circle cx="54" cy="67" r="2" fill="#FFFFFF"/><circle cx="51" cy="86" r="1.8" fill="#FFFFFF"/></svg>"##,
    },
    Sticker {
        id: "labo-ballon",
        label: "Ballon",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 110.0,
        height: 125.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M22 76 A28 28 0 0 0 78 76 Z" fill="#F59E0B" opacity="0.48"/><path d="M40 12 H60 M42 14 V44 A28 28 0 1 0 58 44 V14" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/><line x1="22" y1="76" x2="78" y2="76" stroke="#B45309" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "labo-fiole-jaugee",
        label: "Fiole jaugée",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 105.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M44 42 H56 V58 L76 92 Q80 106 66 108 H34 Q20 106 24 92 L44 58 Z" fill="#3B82F6" opacity="0.38"/><polygon points="44,8 56,8 54,18 46,18" fill="#60A5FA" stroke="#1E293B" stroke-width="2"/><path d="M43 18 H57 M44 18 V58 L24 92 Q20 108 34 108 H66 Q80 108 76 92 L56 58 V18" fill="none" stroke="#1E293B" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"/><line x1="40" y1="42" x2="60" y2="42" stroke="#DC2626" stroke-width="2.4"/></svg>"##,
    },
    Sticker {
        id: "labo-entonnoir",
        label: "Entonnoir",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 105.0,
        height: 120.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><polygon points="20,26 80,26 56,66 44,66" fill="#E0F2FE"/><polygon points="25,20 75,20 50,58" fill="#FFFFFF" stroke="#64748B" stroke-width="1.6" stroke-dasharray="3 2"/><path d="M16 26 H84 L55 68 V106 L45 100 V68 Z" fill="none" stroke="#1E293B" stroke-width="3" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "labo-ampoule-decanter",
        label: "Ampoule à décanter",
        category: Category::Labo,
        lab_group: Some(LabGroup::Verrerie),
        width: 100.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M27 52 H73 C73 66 57 78 55 82 H45 C43 78 27 66 27 52 Z" fill="#3B82F6" opacity="0.45"/><path d="M27 52 H73 C73 42 63 36 56 32 H44 C37 36 27 42 27 52 Z" fill="#FACC15" opacity="0.55"/><rect x="44" y="8" width="12" height="10" rx="2" fill="#94A3B8" stroke="#1E293B" stroke-width="2"/><path d="M44 18 H56 V32 C68 38 74 44 74 52 C74 66 56 78 55 84 V108 L45 104 V84 C44 78 26 66 26 52 C26 44 32 38 44 32 Z" fill="none" stroke="#1E293B" stroke-width="2.8" stroke-linejoin="round"/><rect x="38" y="88" width="24" height="6" rx="2" fill="#2563EB" stroke="#1E293B" stroke-width="1.8"/></svg>"##,
    },
    Sticker {
        id: "labo-pipette",
        label: "Pipette",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 85.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><ellipse cx="50" cy="24" rx="12" ry="15" fill="#EF4444" stroke="#991B1B" stroke-width="2.4"/><rect x="45" y="54" width="10" height="36" fill="#38BDF8" opacity="0.5"/><path d="M45 38 V90 L49 110 H51 L55 90 V38 Z" fill="none" stroke="#1E293B" stroke-width="2.4" stroke-linejoin="round"/><line x1="45" y1="48" x2="52" y2="48" stroke="#1E293B" stroke-width="1.5"/><line x1="45" y1="58" x2="52" y2="58" stroke="#1E293B" stroke-width="1.5"/><line x1="45" y1="68" x2="52" y2="68" stroke="#1E293B" stroke-width="1.5"/><line x1="45" y1="78" x2="52" y2="78" stroke="#1E293B" stroke-width="1.5"/></svg>"##,
    },
    Sticker {
        id: "labo-burette",
        label: "Burette",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 90.0,
        height: 135.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><rect x="44" y="34" width="12" height="54" fill="#60A5FA" opacity="0.45"/><path d="M40 12 L44 16 V88 L49 102 H51 L56 88 V16 L60 12" fill="none" stroke="#1E293B" stroke-width="2.6" stroke-linejoin="round"/><rect x="35" y="74" width="30" height="6" rx="2" fill="#2563EB" stroke="#1E293B" stroke-width="1.8"/><path d="M50 106 Q54 112 50 116 Q46 112 50 106 Z" fill="#38BDF8"/><line x1="44" y1="24" x2="52" y2="24" stroke="#1E293B" stroke-width="1.6"/><line x1="44" y1="36" x2="52" y2="36" stroke="#1E293B" stroke-width="1.6"/><line x1="44" y1="48" x2="52" y2="48" stroke="#1E293B" stroke-width="1.6"/><line x1="44" y1="60" x2="52" y2="60" stroke="#1E293B" stroke-width="1.6"/></svg>"##,
    },
    Sticker {
        id: "labo-bec-bunsen",
        label: "Bec Bunsen",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 100.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M50 10 C60 22 61 36 50 44 C39 36 40 22 50 10 Z" fill="#F97316" opacity="0.8"/><path d="M50 20 C56 28 56 37 50 44 C44 37 44 28 50 20 Z" fill="#38BDF8"/><rect x="43" y="44" width="14" height="50" fill="#94A3B8" stroke="#1E293B" stroke-width="2.4"/><rect x="41" y="78" width="18" height="8" rx="1.5" fill="#64748B" stroke="#1E293B" stroke-width="2"/><polygon points="24,108 76,108 66,94 34,94" fill="#475569" stroke="#1E293B" stroke-width="2.4" stroke-linejoin="round"/></svg>"##,
    },
    Sticker {
        id: "labo-balance",
        label: "Balance",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 120.0,
        height: 105.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 100"><rect x="24" y="24" width="72" height="8" rx="3" fill="#CBD5E1" stroke="#334155" stroke-width="2.4"/><rect x="52" y="32" width="16" height="8" fill="#64748B"/><rect x="14" y="40" width="92" height="46" rx="8" fill="#E2E8F0" stroke="#1E293B" stroke-width="2.8"/><rect x="28" y="52" width="64" height="22" rx="4" fill="#0F172A" stroke="#334155" stroke-width="2"/><line x1="38" y1="63" x2="72" y2="63" stroke="#4ADE80" stroke-width="3" stroke-linecap="round"/><circle cx="82" cy="63" r="2.5" fill="#4ADE80"/></svg>"##,
    },
    Sticker {
        id: "labo-thermometre",
        label: "Thermomètre",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 85.0,
        height: 130.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><rect x="43" y="12" width="14" height="76" rx="7" fill="#F8FAFC" stroke="#1E293B" stroke-width="2.8"/><circle cx="50" cy="96" r="14" fill="#EF4444" stroke="#1E293B" stroke-width="2.8"/><rect x="47" y="38" width="6" height="52" rx="3" fill="#EF4444"/><line x1="59" y1="24" x2="68" y2="24" stroke="#1E293B" stroke-width="2"/><line x1="59" y1="36" x2="66" y2="36" stroke="#1E293B" stroke-width="2"/><line x1="59" y1="48" x2="68" y2="48" stroke="#1E293B" stroke-width="2"/><line x1="59" y1="60" x2="66" y2="60" stroke="#1E293B" stroke-width="2"/><line x1="59" y1="72" x2="68" y2="72" stroke="#1E293B" stroke-width="2"/></svg>"##,
    },
    Sticker {
        id: "labo-pissette",
        label: "Pissette",
        category: Category::Labo,
        lab_group: Some(LabGroup::Mesure),
        width: 105.0,
        height: 125.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><path d="M56 36 C58 20 68 14 82 22" fill="none" stroke="#1E293B" stroke-width="3" stroke-linecap="round"/><path d="M84 26 Q88 32 84 36 Q80 32 84 26 Z" fill="#38BDF8"/><rect x="40" y="34" width="20" height="10" rx="2" fill="#2563EB" stroke="#1E293B" stroke-width="2"/><rect x="30" y="44" width="40" height="62" rx="8" fill="#F1F5F9" stroke="#1E293B" stroke-width="2.8"/><rect x="32" y="64" width="36" height="40" rx="5" fill="#60A5FA" opacity="0.45"/></svg>"##,
    },
    Sticker {
        id: "labo-sgh-inflammable",
        label: "Inflammable",
        category: Category::Labo,
        lab_group: Some(LabGroup::Securite),
        width: 90.0,
        height: 90.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><polygon points="50,6 94,50 50,94 6,50" fill="#FFFFFF" stroke="#DC2626" stroke-width="6" stroke-linejoin="miter"/><path d="M51 24 C57 35 66 43 66 56 C66 66 59 73 50 73 C40 73 34 65 34 56 C34 46 41 40 43 35 C44 41 47 43 49 38 C49 32 50 28 51 24 Z" fill="#111827"/></svg>"##,
    },
    Sticker {
        id: "labo-sgh-corrosif",
        label: "Corrosif",
        category: Category::Labo,
        lab_group: Some(LabGroup::Securite),
        width: 90.0,
        height: 90.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><polygon points="50,6 94,50 50,94 6,50" fill="#FFFFFF" stroke="#DC2626" stroke-width="6" stroke-linejoin="miter"/><rect x="28" y="28" width="16" height="7" transform="rotate(-30 36 31)" fill="#111827"/><rect x="56" y="28" width="16" height="7" transform="rotate(30 64 31)" fill="#111827"/><circle cx="38" cy="48" r="2.5" fill="#111827"/><circle cx="62" cy="48" r="2.5" fill="#111827"/><rect x="28" y="60" width="18" height="8" rx="1" fill="#111827"/><rect x="52" y="59" width="20" height="8" rx="3" fill="#111827"/></svg>"##,
    },
    Sticker {
        id: "labo-sgh-toxique",
        label: "Toxique",
        category: Category::Labo,
        lab_group: Some(LabGroup::Securite),
        width: 90.0,
        height: 90.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><polygon points="50,6 94,50 50,94 6,50" fill="#FFFFFF" stroke="#DC2626" stroke-width="6" stroke-linejoin="miter"/><circle cx="50" cy="42" r="13" fill="#111827"/><rect x="43" y="51" width="14" height="7" rx="2" fill="#111827"/><circle cx="45" cy="41" r="3" fill="#FFFFFF"/><circle cx="55" cy="41" r="3" fill="#FFFFFF"/><line x1="33" y1="68" x2="67" y2="57" stroke="#111827" stroke-width="4.2" stroke-linecap="round"/><line x1="33" y1="57" x2="67" y2="68" stroke="#111827" stroke-width="4.2" stroke-linecap="round"/></svg>"##,
    },
    Sticker {
        id: "labo-sgh-nocif",
        label: "Nocif / Irritant",
        category: Category::Labo,
        lab_group: Some(LabGroup::Securite),
        width: 90.0,
        height: 90.0,
        svg: r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><polygon points="50,6 94,50 50,94 6,50" fill="#FFFFFF" stroke="#DC2626" stroke-width="6" stroke-linejoin="miter"/><line x1="50" y1="27" x2="50" y2="55" stroke="#111827" stroke-width="8" stroke-linecap="round"/><circle cx="50" cy="68" r="4.5" fill="#111827"/></svg>"##,
    },
];

pub fn find(id: &str) -> Option<&'static Sticker> {
    SMILEYS.iter().chain(EMOJIS).chain(LABO).find(|s| s.id == id)
}

pub fn seasonal_smileys(season: Season) -> Vec<&'static Sticker> {
    let ids: &[&str] = match season {
        Season::Printemps => &["smiley-fleur", "smiley-sourire", "smiley-etoiles", "smiley-fete", "smiley-clin-doeil", "smiley-ange"],
        Season::Ete => &["smiley-plage", "smiley-cool", "smiley-rire", "smiley-fete", "smiley-etoiles", "smiley-bisou"],
        Season::Automne => &["smiley-echarpe", "smiley-reflechi", "smiley-sourire", "smiley-sueur", "smiley-clin-doeil", "smiley-dort"],
        Season::Hiver => &["smiley-bonnet-hiver", "smiley-malade", "smiley-fete", "smiley-etoiles", "smiley-dort", "smiley-sourire"],
    };
    ids.iter().filter_map(|id| find(id)).collect()
}

pub fn seasonal_emojis(season: Season) -> Vec<&'static Sticker> {
    let ids: &[&str] = match season {
        Season::Printemps => &["emoji-fleur-cerisier", "emoji-papillon", "emoji-arc-en-ciel", "emoji-abeille", "emoji-plante", "emoji-soleil"],
        Season::Ete => &["emoji-soleil", "emoji-glace", "emoji-palmier", "emoji-parasol", "emoji-pasteque", "emoji-arc-en-ciel"],
        Season::Automne => &["emoji-feuille-automne", "emoji-citrouille", "emoji-champignon", "emoji-gland", "emoji-parapluie", "emoji-tasse-chaude"],
        Season::Hiver => &["emoji-flocon", "emoji-bonhomme-neige", "emoji-sapin", "emoji-cadeau", "emoji-tasse-chaude", "emoji-lune"],
    };
    ids.iter().filter_map(|id| find(id)).collect()
}

pub fn preview_uri(sticker: &Sticker) -> String {
    format!("bytes://ergonote-sticker-{}.svg", sticker.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_stickers_are_unique_and_valid_svg() {
        let all: Vec<&Sticker> = SMILEYS.iter().chain(EMOJIS).chain(LABO).collect();
        let mut ids = HashSet::new();
        assert_eq!(SMILEYS.len(), 36);
        assert_eq!(EMOJIS.len(), 42);
        assert_eq!(LABO.len(), 18);
        for s in all {
            assert!(ids.insert(s.id), "duplicate sticker id: {}", s.id);
            assert!(!s.label.is_empty(), "missing label for {}", s.id);
            assert!(s.width > 0.0 && s.height > 0.0, "bad dimensions for {}", s.id);
            assert!(s.svg.starts_with("<svg") && s.svg.ends_with("</svg>"), "bad svg markup for {}", s.id);
            assert_eq!(find(s.id).map(|found| found.id), Some(s.id));
        }
    }

    #[test]
    fn seasonal_rows_and_boundaries_work() {
        assert_eq!(season_for_month_day(3, 20), Season::Printemps);
        assert_eq!(season_for_month_day(6, 20), Season::Printemps);
        assert_eq!(season_for_month_day(6, 21), Season::Ete);
        assert_eq!(season_for_month_day(9, 21), Season::Ete);
        assert_eq!(season_for_month_day(9, 22), Season::Automne);
        assert_eq!(season_for_month_day(11, 27), Season::Automne);
        assert_eq!(season_for_month_day(12, 20), Season::Automne);
        assert_eq!(season_for_month_day(12, 21), Season::Hiver);
        assert_eq!(season_for_month_day(1, 15), Season::Hiver);
        assert_eq!(season_for_month_day(3, 19), Season::Hiver);
        for season in [Season::Printemps, Season::Ete, Season::Automne, Season::Hiver] {
            assert_eq!(seasonal_smileys(season).len(), 6);
            assert_eq!(seasonal_emojis(season).len(), 6);
        }
    }
}
