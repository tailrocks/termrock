//! Validators for the named cell grammars a form or grid can reuse.
//!
//! Each helper checks one documented grammar over the exact text it is given.
//! None trims, normalises or rewrites: the caller keeps the user's spelling.
//! They return `Ok(())` so a caller can attach the validated text to whichever
//! value type it uses. [`crate::widgets::grid::default_validator`] trims, then
//! calls these for `Json`, `Id` and `Timestamp` columns. An application that
//! wants a looser or different grammar supplies its own
//! [`crate::widgets::grid::cell_validator`].

use super::FieldError;

/// Accepts exactly one complete JSON object or array and nothing else.
///
/// Scalars, trailing text, invalid escapes, unfinished content and empty
/// input are rejected. The text is parsed with `serde_json`'s default
/// recursion limit, so a container nested deeper than that limit (128 levels)
/// is also rejected; use a custom parser to accept it. The input is never
/// reserialised, so key order and whitespace stay the user's.
pub fn json_container(text: &str) -> Result<(), FieldError> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(serde_json::Value::Object(_) | serde_json::Value::Array(_)) => Ok(()),
        Ok(_) => Err(FieldError::new(
            "Must be a JSON object or array, not a bare value",
        )),
        Err(e) => Err(FieldError::new(format!(
            "Must be a JSON object or array: {e}"
        ))),
    }
}

/// Accepts a UUID in canonical 8-4-4-4-12 grouping.
///
/// Exactly 36 ASCII bytes, hyphens at offsets 8, 13, 18 and 23, hexadecimal
/// digits of either case elsewhere. The version and variant bits are not
/// constrained. "Canonical" describes the grouping only: the helper never
/// lower-cases, so the user's spelling is preserved. Braced, compact and URN
/// forms are rejected.
pub fn canonical_uuid(text: &str) -> Result<(), FieldError> {
    let b = text.as_bytes();
    let grouped = b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        });
    if grouped {
        Ok(())
    } else {
        Err(FieldError::new(
            "Must be a UUID like 123e4567-e89b-12d3-a456-426614174000",
        ))
    }
}

const TIMESTAMP_FORM: &str = "Use YYYY-MM-DD or YYYY-MM-DD HH:MM:SS[.fraction][Z|+HH[:MM]]";

/// Accepts `YYYY-MM-DD`, optionally followed by `T` or a space and
/// `HH:MM:SS`, an optional `.` and 1 to 9 fraction digits, and an optional
/// `Z`, `+HH`, `-HH`, `+HH:MM` or `-HH:MM` offset.
///
/// Years run 0001 to 9999 with Gregorian leap rules, hours 00 to 23, minutes
/// and seconds 00 to 59 (no leap second). The offset is a plain fixed offset:
/// no named zone, no conversion, and a missing offset does not mean local
/// time. An offset needs a time before it; lowercase `z`, missing seconds and
/// trailing text are rejected.
pub fn date_or_timestamp(text: &str) -> Result<(), FieldError> {
    timestamp_problem(text).map_or(Ok(()), |why| Err(FieldError::new(why)))
}

fn timestamp_problem(text: &str) -> Option<String> {
    if !text.is_ascii() || text.len() < 10 {
        return Some(TIMESTAMP_FORM.to_owned());
    }
    let b = text.as_bytes();
    let digits = |from: usize, to: usize| -> Option<u32> {
        let part = &b[from..to];
        part.iter()
            .all(u8::is_ascii_digit)
            .then(|| part.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
    };
    if b[4] != b'-' || b[7] != b'-' {
        return Some(TIMESTAMP_FORM.to_owned());
    }
    let (Some(year), Some(month), Some(day)) = (digits(0, 4), digits(5, 7), digits(8, 10)) else {
        return Some(TIMESTAMP_FORM.to_owned());
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return Some(format!("Month {month:02} does not exist")),
    };
    if year == 0 {
        return Some("Year must be 0001 to 9999".to_owned());
    }
    if day == 0 || day > days {
        return Some(format!(
            "{year:04}-{month:02} has no day {day:02} ({days} days)"
        ));
    }
    if b.len() == 10 {
        return None;
    }
    if b.len() < 19 || !matches!(b[10], b'T' | b' ') || b[13] != b':' || b[16] != b':' {
        return Some(TIMESTAMP_FORM.to_owned());
    }
    let (Some(hour), Some(minute), Some(second)) = (digits(11, 13), digits(14, 16), digits(17, 19))
    else {
        return Some(TIMESTAMP_FORM.to_owned());
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Some("Time must be 00:00:00 to 23:59:59".to_owned());
    }
    let mut at = 19;
    if b.get(at) == Some(&b'.') {
        at += 1;
        let begin = at;
        while b.get(at).is_some_and(|c| c.is_ascii_digit()) {
            at += 1;
        }
        if !(1..=9).contains(&(at - begin)) {
            return Some("Fraction of a second takes 1 to 9 digits".to_owned());
        }
    }
    let zone = &b[at..];
    if zone.is_empty() || zone == b"Z" {
        return None;
    }
    let offset_ok = matches!(zone[0], b'+' | b'-')
        && match zone.len() {
            3 => digits(at + 1, at + 3).is_some_and(|h| h <= 23),
            6 => {
                zone[3] == b':'
                    && digits(at + 1, at + 3).is_some_and(|h| h <= 23)
                    && digits(at + 4, at + 6).is_some_and(|m| m <= 59)
            }
            _ => false,
        };
    (!offset_ok).then(|| "Offset must be Z, +HH, -HH, +HH:MM or -HH:MM".to_owned())
}
