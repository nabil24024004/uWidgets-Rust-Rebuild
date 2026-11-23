//! The Rust port of `Clock/`.
//!
//! Faithful port of `Clock/ClockLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn utf16_prefix(s: &str, max_units: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().take(max_units).collect();
    String::from_utf16_lossy(&units)
}

fn csharp_upper_invariant(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            // C# char.ToUpperInvariant maps 1:1 on UTF-16 code units (keeps sharp s unchanged)
            '\u{00DF}' => out.push(ch),
            '\u{0131}' => out.push('I'),
            _ => {
                let mut up = ch.to_uppercase();
                if up.clone().count() == 1 {
                    if let Some(c) = up.next() {
                        out.push(c);
                    }
                } else {
                    out.push(ch);
                }
            }
        }
    }
    out
}

/// Port of `SecondsAngle` (`Clock/ClockLogic.cs:7`).
pub fn seconds_angle(second: i64, millisecond: i64) -> Result<f64, String> {
    let s = second as i32;
    let ms = millisecond as i32;
    Ok((f64::from(s) + f64::from(ms) / 1000.0) * 6.0)
}

/// Port of `MinutesAngle` (`Clock/ClockLogic.cs:12`).
pub fn minutes_angle(minute: i64, second: i64) -> Result<f64, String> {
    let m = minute as i32;
    let s = second as i32;
    Ok((f64::from(m) + f64::from(s) / 60.0) * 6.0)
}

/// Port of `HoursAngle` (`Clock/ClockLogic.cs:17`).
pub fn hours_angle(hour: i64, minute: i64) -> Result<f64, String> {
    let h = hour as i32;
    let m = minute as i32;
    Ok((f64::from(h % 12) + f64::from(m) / 60.0) * 30.0)
}

/// Port of `ClockHandEndY` (`Clock/ClockLogic.cs:22`).
pub fn clock_hand_end_y(length: i64) -> Result<i64, String> {
    Ok(i64::from(420_i32.wrapping_sub(length as i32)))
}

/// Port of `ClockHandPath` (`Clock/ClockLogic.cs:27`).
pub fn clock_hand_path(length: i64) -> Result<String, String> {
    Ok(format!(
        "M 500,420 V {}",
        420_i32.wrapping_sub(length as i32)
    ))
}

/// Port of `WorldIsWideLayout` (`Clock/ClockLogic.cs:32`).
pub fn world_is_wide_layout(width: f64, height: f64) -> Result<bool, String> {
    if height <= 0.0 {
        return Ok(false);
    }
    Ok((width / height) >= 1.5)
}

/// Port of `CityFullName` (`Clock/ClockLogic.cs:41`).
pub fn city_full_name(display_name: String) -> Result<String, String> {
    let after_offset = display_name.rsplit(") ").next().unwrap_or("");
    let first_comma = after_offset.split(", ").next().unwrap_or("");
    let before_paren = first_comma.split('(').next().unwrap_or("");
    Ok(before_paren.to_string())
}

/// Port of `CityShortName` (`Clock/ClockLogic.cs:49`).
pub fn city_short_name(display_name: String) -> Result<String, String> {
    let full = city_full_name(display_name)?;
    if utf16_len(&full) <= 11 {
        return Ok(full);
    }
    Ok(format!("{}\u{2026}", utf16_prefix(&full, 9)))
}

/// Port of `CityAbbreviation` (`Clock/ClockLogic.cs:59`).
pub fn city_abbreviation(display_name: String, is_local: bool) -> Result<String, String> {
    if is_local {
        return Ok(String::new());
    }
    let tail = display_name.rsplit(") ").next().unwrap_or("");
    if utf16_len(tail) < 3 {
        return Ok(csharp_upper_invariant(tail));
    }
    Ok(csharp_upper_invariant(&utf16_prefix(tail, 3)))
}

/// Port of `FormatTimeZoneDiff` (`Clock/ClockLogic.cs:74`).
pub fn format_time_zone_diff(diff_minutes: i64) -> Result<String, String> {
    let dm = diff_minutes as i32;
    let abs_mins = dm.checked_abs().ok_or_else(|| {
        "System.OverflowException: Negating the minimum value of a twos complement number is invalid."
            .to_string()
    })?;
    let hours = dm / 60;
    let span_hours = (abs_mins / 60) % 24;
    let span_mins = abs_mins % 60;
    if hours > 0 {
        Ok(format!("+{span_hours:02}:{span_mins:02}"))
    } else if hours < 0 {
        Ok(format!("-{span_hours:02}:{span_mins:02}"))
    } else {
        Ok("00:00".to_string())
    }
}

/// Port of `RelativeDayKey` (`Clock/ClockLogic.cs:89`).
pub fn relative_day_key(day_delta: i64) -> Result<String, String> {
    let d = day_delta as i32;
    if d < 0 {
        Ok("Clock_Yesterday".to_string())
    } else if d > 0 {
        Ok("Clock_Tomorrow".to_string())
    } else {
        Ok("Clock_Today".to_string())
    }
}

/// Port of `FormatDigitalTime` (`Clock/ClockLogic.cs:102`).
pub fn format_digital_time(
    hour24: i64,
    minute: i64,
    second: i64,
    use24_hours: bool,
    show_seconds: bool,
) -> Result<String, String> {
    let h = ((hour24 as i32 % 24) + 24) % 24;
    let m = (minute as i32).clamp(0, 59);
    let s = (second as i32).clamp(0, 59);
    let sec = if show_seconds {
        format!(":{s:02}")
    } else {
        String::new()
    };
    if use24_hours {
        return Ok(format!("{h:02}:{m:02}{sec}"));
    }
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let ampm = if h < 12 { "AM" } else { "PM" };
    Ok(format!("{h12:02}:{m:02}{sec} {ampm}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_angles_and_formatting() {
        assert_eq!(hours_angle(15, 30).ok(), Some(105.0));
        assert_eq!(minutes_angle(30, 0).ok(), Some(180.0));
        assert_eq!(seconds_angle(15, 500).ok(), Some(93.0));
        assert_eq!(
            clock_hand_path(190).ok().as_deref(),
            Some("M 500,420 V 230")
        );
        assert_eq!(format_time_zone_diff(360).ok().as_deref(), Some("+06:00"));
        assert_eq!(format_time_zone_diff(-300).ok().as_deref(), Some("-05:00"));
        assert_eq!(format_time_zone_diff(0).ok().as_deref(), Some("00:00"));
        assert!(format_time_zone_diff(i64::from(i32::MIN)).is_err());
        assert_eq!(
            format_digital_time(0, 5, 9, false, true).ok().as_deref(),
            Some("12:05:09 AM")
        );
    }
}
