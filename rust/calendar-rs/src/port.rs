//! The Rust port of `Calendar/`.
//!
//! Faithful port of `Calendar/CalendarLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

fn csharp_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() || b.is_sign_negative() {
            -0.0
        } else {
            0.0
        }
    } else if a <= b {
        a
    } else {
        b
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Port of `DaysInMonth` (`Calendar/CalendarLogic.cs:7`).
pub fn days_in_month(year: i64, month: i64) -> Result<i64, String> {
    let y = year as i32;
    let m = month as i32;
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) {
        return Ok(0);
    }
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    Ok(i64::from(days))
}

/// Port of `EmptyDaysCount` (`Calendar/CalendarLogic.cs:16`).
pub fn empty_days_count(
    first_of_month_day_of_week: i64,
    first_day_of_week: i64,
) -> Result<i64, String> {
    let dow = ((first_of_month_day_of_week as i32 % 7) + 7) % 7;
    let first = ((first_day_of_week as i32 % 7) + 7) % 7;
    Ok(i64::from((dow + first + 5) % 7))
}

/// Port of `IsWeekend` (`Calendar/CalendarLogic.cs:23`).
pub fn is_weekend(day_of_week: i64) -> Result<bool, String> {
    let d = ((day_of_week as i32 % 7) + 7) % 7;
    Ok(d == 0 || d == 6)
}

/// Port of `WeekdayOrder` (`Calendar/CalendarLogic.cs:29`).
pub fn weekday_order(first_day_of_week: i64) -> Result<Vec<i64>, String> {
    let offset = ((first_day_of_week as i32 % 7) + 7) % 7;
    Ok((0..7).map(|i| i64::from((i + offset) % 7)).collect())
}

/// Port of `MonthTextSize` (`Calendar/CalendarLogic.cs:40`).
pub fn month_text_size(width: f64, height: f64) -> Result<f64, String> {
    Ok(csharp_min(width, height) / 12.0)
}

/// Port of `MonthMarginLeft` (`Calendar/CalendarLogic.cs:45`).
pub fn month_margin_left(width: f64, height: f64) -> Result<f64, String> {
    Ok(0.0 - csharp_min(width, height) / 30.0)
}

/// Port of `StripTrailingDot` (`Calendar/CalendarLogic.cs:50`).
pub fn strip_trailing_dot(input: String) -> Result<String, String> {
    Ok(input.replace('.', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_core_cases() {
        assert_eq!(days_in_month(2024, 2).ok(), Some(29));
        assert_eq!(days_in_month(2026, 2).ok(), Some(28));
        assert_eq!(empty_days_count(1, 1).ok(), Some(0)); // Mon start, 1st is Mon
        assert_eq!(empty_days_count(0, 1).ok(), Some(6)); // Mon start, 1st is Sun
        assert_eq!(is_weekend(0).ok(), Some(true));
        assert_eq!(is_weekend(6).ok(), Some(true));
        assert_eq!(is_weekend(3).ok(), Some(false));
        assert_eq!(weekday_order(1).ok(), Some(vec![1, 2, 3, 4, 5, 6, 0]));
    }
}
