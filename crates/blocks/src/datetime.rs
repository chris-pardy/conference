//! The lexicon `datetime` format exactly as `@atproto/lexicon` 0.7 checks it:
//! `isDatetimeStringLenient` from `@atproto/syntax`, which accepts a string
//! when either `iso-datestring-validator`'s `isValidISODateString` or the
//! strict atproto (RFC 3339) check accepts it.
//!
//! Both are ported rule for rule, quirks included (a `T24:00` hour, a
//! comma in the month class, any character before fractional seconds), so
//! the appview and the PWA agree on every record.

use std::sync::LazyLock;

use regex::Regex;

pub(crate) fn is_datetime_lenient(s: &str) -> bool {
    is_valid_iso_date_string(s) || is_datetime_strict(s)
}

// --- iso-datestring-validator 2.2.2 ----------------------------------------

/// The first character that isn't an ASCII digit, as the library's `\D` finds it.
fn first_non_digit(s: &str) -> Option<char> {
    s.chars().find(|c| !c.is_ascii_digit())
}

/// The library builds its regexes with the separator spliced in unescaped. A
/// `.` then matches anything; other regex metacharacters make the pattern
/// either throw (caught upstream, so invalid) or mean something else, which
/// we treat as invalid too.
fn sep_matches(sep: Option<char>, c: Option<char>) -> bool {
    match sep {
        Some('.') => c.is_some(),
        other => c == other,
    }
}

fn sep_usable(sep: Option<char>) -> bool {
    !matches!(
        sep,
        Some('\\' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}')
    )
}

fn digits(chars: &[char]) -> Option<u32> {
    if chars.iter().all(char::is_ascii_digit) {
        chars.iter().try_fold(0u32, |n, c| Some(n * 10 + c.to_digit(10)?))
    } else {
        None
    }
}

fn is_valid_iso_date_string(s: &str) -> bool {
    let mut parts = s.split('T');
    let date = parts.next().unwrap_or_default();
    let Some(time) = parts.next() else {
        return false;
    };
    if time.is_empty() {
        return false;
    }
    is_valid_date(date, first_non_digit(date)) && is_valid_time(time, time_separator(time))
}

/// `YYYY-MM-DD` with the separator the date uses (none for `YYYYMMDD`), on the proleptic calendar.
fn is_valid_date(s: &str, sep: Option<char>) -> bool {
    if !sep_usable(sep) {
        return false;
    }
    let chars: Vec<char> = s.chars().collect();
    let w = usize::from(sep.is_some());
    if chars.len() != 8 + 2 * w {
        return false;
    }
    let (y, m, d) = (&chars[0..4], &chars[4 + w..6 + w], &chars[6 + 2 * w..8 + 2 * w]);
    if w == 1 && !(sep_matches(sep, Some(chars[4])) && sep_matches(sep, Some(chars[7]))) {
        return false;
    }
    let Some(year) = digits(y) else {
        return false;
    };
    // Months: 01, 03-09 (and, through the library's `[1,3-9]`, a comma), 10-12, or 02.
    let (m1, m2) = (m[0], m[1]);
    let month_ok = (m1 == '0' && matches!(m2, '1' | '3'..='9' | ','))
        || (m1 == '1' && matches!(m2, '0'..='2'));
    let february = m1 == '0' && m2 == '2';
    let Some(day) = digits(d) else {
        return false;
    };
    if !(1..=31).contains(&day) || !(month_ok || february) {
        return false;
    }
    let short = matches!((m1, m2), ('0', '4' | '6' | '9') | ('1', '1'));
    if short && day == 31 {
        return false;
    }
    if february {
        let leap = if year % 100 == 0 { year != 0 && (year / 100) % 4 == 0 } else { year % 4 == 0 };
        return day <= 28 || (day == 29 && leap);
    }
    true
}

/// The time separator: the first character (not `Z`, `+`, `-` or a digit)
/// that is followed by digits and then itself again, e.g. `:` in `19:00:00`.
fn time_separator(time: &str) -> Option<char> {
    let chars: Vec<char> = time.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_digit() || matches!(c, 'Z' | '+' | '-') {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j > i + 1 && j < chars.len() && chars[j] == c {
            return Some(c);
        }
    }
    None
}

/// `hh:mm[:ss][.fraction]`, then `Z` or an offset from the list of real ones.
fn is_valid_time(time: &str, sep: Option<char>) -> bool {
    if !sep_usable(sep) {
        return false;
    }
    if !time.contains(['Z', '+', '-']) {
        return is_valid_clock(time, sep);
    }
    if time.ends_with('Z') {
        return is_valid_clock(&time.replacen('Z', "", 1), sep);
    }
    let plus = time.contains('+');
    let mut parts = time.split(['+', '-']);
    let clock = parts.next().unwrap_or_default();
    let offset = parts.next().unwrap_or_default();
    is_valid_clock(clock, sep) && is_valid_offset(offset, plus, first_non_digit(offset))
}

fn is_valid_clock(s: &str, sep: Option<char>) -> bool {
    let c: Vec<char> = s.chars().collect();
    let at = |i: usize| c.get(i).copied();
    let w = usize::from(sep.is_some());
    // Hours 00-23, or 24 when the minutes are 00.
    let hour_ok = match (at(0), at(1)) {
        (Some('0' | '1'), Some(d)) => d.is_ascii_digit(),
        (Some('2'), Some('0'..='3')) => true,
        (Some('2'), Some('4')) => {
            (w == 0 || sep_matches(sep, at(2))) && at(2 + w) == Some('0') && at(3 + w) == Some('0')
        }
        _ => false,
    };
    if !hour_ok || (w == 1 && !sep_matches(sep, at(2))) {
        return false;
    }
    let i = 2 + w;
    if !(matches!(at(i), Some('0'..='5')) && at(i + 1).is_some_and(|d| d.is_ascii_digit())) {
        return false;
    }
    let i = i + 2;
    // The rest from `j`: nothing, or an optional fraction: any one character
    // (but a line break, as `.` without the s flag), then 1 to 9 digits.
    let tail_ok = |j: usize| {
        j == c.len()
            || (j < c.len()
                && !matches!(c[j], '\n' | '\r' | '\u{2028}' | '\u{2029}')
                && (1..=9).contains(&(c.len() - j - 1))
                && c[j + 1..].iter().all(char::is_ascii_digit))
    };
    // Optional seconds, 00-59 or 60. As in the regex, a string that fails
    // with seconds may still pass without them (read as a fraction instead).
    let seconds = |k: usize| match (at(k), at(k + 1)) {
        (Some('0'..='5'), Some(d)) => d.is_ascii_digit(),
        (Some('6'), Some('0')) => true,
        _ => false,
    };
    let with_seconds = if w == 1 {
        sep_matches(sep, at(i)) && seconds(i + 1) && tail_ok(i + 3)
    } else {
        seconds(i) && tail_ok(i + 2)
    };
    with_seconds || tail_ok(i)
}

/// The offsets the library accepts: the ones in real use.
fn is_valid_offset(s: &str, plus: bool, sep: Option<char>) -> bool {
    if !sep_usable(sep) {
        return false;
    }
    let c: Vec<char> = s.chars().collect();
    let w = usize::from(sep.is_some());
    if c.len() != 4 + w || (w == 1 && !sep_matches(sep, Some(c[2]))) {
        return false;
    }
    let (h1, h2, m1, m2) = (c[0], c[1], c[2 + w], c[3 + w]);
    if plus {
        // Minutes are :00, :30 or :45, depending on the hour.
        let minutes_ok = matches!((m1, m2), ('0' | '3', '0') | ('4', '5'));
        let h2_ok = match h2 {
            '0' | '3' | '4' | '6' | '9' => matches!(m1, '0' | '3'),
            '1' | '7' => m1 == '0',
            '2' | '8' => matches!(m1, '0' | '4'),
            '5' => matches!(m1, '0' | '3' | '4'),
            _ => false,
        };
        let h1_ok = match h1 {
            '0' => !((h2 == '2' && m1 == '4') || (h2 == '0' && m1 == '3')),
            '1' => match h2 {
                '0' | '1' => true,
                '2' => matches!(m1, '0' | '4'),
                '3' | '4' => m1 == '0',
                _ => false,
            },
            _ => false,
        };
        minutes_ok && h2_ok && h1_ok
    } else {
        let h1_ok = (h1 == '0' && h2 != '0') || (h1 == '1' && matches!(h2, '0'..='2'));
        let rest_ok = match h2 {
            '3' | '9' => matches!(m1, '0' | '3'),
            '0'..='2' | '4'..='8' => m1 == '0',
            _ => false,
        };
        h1_ok && rest_ok && m2 == '0'
    }
}

// --- @atproto/syntax strict datetime ----------------------------------------

static STRICT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^([0-9]{4})-(0[1-9]|1[012])-([0-2][0-9]|3[01])T([0-1][0-9]|2[0-3]):([0-5][0-9]):([0-5][0-9]|60)(\.[0-9]+)?(Z|([+-])((?:[0-1][0-9]|2[0-3])):([0-5][0-9]))$",
    )
    .unwrap()
});

/// RFC 3339 with `Z` or `±hh:mm`, that JavaScript's `Date` parses to a year from 0 to 9999 in UTC.
fn is_datetime_strict(s: &str) -> bool {
    if s.len() > 64 || s.ends_with("-00:00") {
        return false;
    }
    let Some(caps) = STRICT.captures(s) else {
        return false;
    };
    let num = |i: usize| caps.get(i).map_or(0, |m| m.as_str().parse::<i64>().unwrap_or(0));
    // V8 rejects a leap second, and rolls a day past the month's end into the next month.
    if num(6) == 60 {
        return false;
    }
    let (year, month, day) = (num(1), num(2), num(3));
    let offset = match caps.get(9).map(|m| m.as_str()) {
        Some("-") => -(num(10) * 60 + num(11)),
        Some(_) => num(10) * 60 + num(11),
        None => 0,
    };
    let minutes = num(4) * 60 + num(5) - offset;
    // Only the first and last days of a year can cross into another year.
    let year_utc = if month == 1 && day == 1 && minutes < 0 {
        year - 1
    } else if month == 12 && day == 31 && minutes >= 24 * 60 {
        year + 1
    } else {
        year
    };
    (0..=9999).contains(&year_utc)
}
