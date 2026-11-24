//! Smiley categories that depend on the date: a holiday only around its dates, a season only in
//! its season. Pure functions so they are easy to test. No variation selectors (U+FE0F), so
//! every symbol has a glyph in the bundled fonts.

/// A named group of symbols.
pub type Category = (&'static str, &'static [&'static str]);

const HALLOWEEN: Category = ("Halloween", &["\u{1F383}", "\u{1F47B}", "\u{1F577}", "\u{1F987}", "\u{1F480}"]);
const NOEL: Category = ("No\u{eb}l", &["\u{1F384}", "\u{1F385}", "\u{2744}", "\u{1F381}", "\u{26C4}", "\u{1F514}"]);
const PAQUES: Category = ("P\u{e2}ques", &["\u{1F423}", "\u{1F430}", "\u{1F95A}", "\u{1F337}", "\u{1F414}"]);
const NOUVEL_AN: Category = ("Nouvel An", &["\u{1F389}", "\u{1F38A}", "\u{2728}", "\u{1F387}"]);
const HIVER: Category = ("Hiver", &["\u{2744}", "\u{26C4}", "\u{1F9E3}", "\u{2615}", "\u{1F9E4}"]);
const PRINTEMPS: Category = ("Printemps", &["\u{1F337}", "\u{1F33B}", "\u{1F98B}", "\u{1F331}", "\u{1F41D}"]);
const ETE: Category = ("\u{c9}t\u{e9}", &["\u{2600}", "\u{1F3D6}", "\u{1F366}", "\u{1F576}", "\u{1F349}"]);
const AUTOMNE: Category = ("Automne", &["\u{1F342}", "\u{1F341}", "\u{1F344}", "\u{1F330}", "\u{2614}"]);

/// Easter Sunday (month, day) in the Gregorian calendar (Meeus/Jones/Butcher).
pub fn easter(year: i32) -> (u8, u8) {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    (month as u8, day as u8)
}

/// Day of the year, 1-based, ignoring leap years except for Easter's own year.
fn ordinal(year: i32, month: u8, day: u8) -> i32 {
    const DAYS: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let before: i32 = DAYS.iter().take(usize::from(month.saturating_sub(1))).sum();
    before + i32::from(leap && month > 2) + i32::from(day)
}

/// Civil date (year, month, day) for a count of days since 1970-01-01 (Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as i32, month as u8, day as u8)
}

/// Today's local date, from the system clock and the offset the app already knows.
pub fn today() -> (i32, u8, u8) {
    let secs = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    civil_from_days((secs + i64::from(notecraft_model::time::local_offset())).div_euclid(86_400))
}

/// Categories to show on this date: the holiday groups in their window, then the season.
pub fn seasonal_categories(year: i32, month: u8, day: u8) -> Vec<Category> {
    let mut out = Vec::new();
    let md = (month, day);
    if ((10, 15)..=(11, 2)).contains(&md) {
        out.push(HALLOWEEN);
    }
    if month == 12 {
        out.push(NOEL);
    }
    let (em, ed) = easter(year);
    let diff = ordinal(year, month, day) - ordinal(year, em, ed);
    if (-14..=7).contains(&diff) {
        out.push(PAQUES);
    }
    if month == 1 && day <= 10 {
        out.push(NOUVEL_AN);
    }
    out.push(match month {
        12 | 1 | 2 => HIVER,
        3..=5 => PRINTEMPS,
        6..=8 => ETE,
        _ => AUTOMNE,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(y: i32, m: u8, d: u8) -> Vec<&'static str> {
        seasonal_categories(y, m, d).into_iter().map(|c| c.0).collect()
    }

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_781), (2026, 11, 24));
    }

    #[test]
    fn easter_dates_are_right() {
        assert_eq!(easter(2025), (4, 20));
        assert_eq!(easter(2026), (4, 5));
    }

    #[test]
    fn holidays_show_only_in_their_window() {
        assert!(names(2026, 10, 31).contains(&"Halloween"));
        assert!(!names(2026, 7, 15).contains(&"Halloween"));
        assert!(names(2026, 12, 20).contains(&"No\u{eb}l"));
        assert!(!names(2026, 11, 24).contains(&"No\u{eb}l"));
        assert!(names(2026, 4, 1).contains(&"P\u{e2}ques"));
        assert!(!names(2026, 5, 1).contains(&"P\u{e2}ques"));
        assert!(names(2026, 1, 5).contains(&"Nouvel An"));
    }

    #[test]
    fn one_season_at_a_time() {
        assert!(names(2026, 1, 20).contains(&"Hiver"));
        assert!(!names(2026, 7, 1).contains(&"Hiver"));
        assert!(names(2026, 7, 1).contains(&"\u{c9}t\u{e9}"));
        assert!(names(2026, 11, 24).contains(&"Automne"));
    }

    #[test]
    fn no_variation_selectors() {
        for m in 1..=12 {
            for (_, syms) in seasonal_categories(2026, m, 15) {
                assert!(syms.iter().all(|s| !s.contains('\u{FE0F}')));
            }
        }
    }
}
