//! The Rust port of `Weather/`.
//!
//! Faithful port of `Weather/WeatherLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

fn csharp_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a <= b {
        a
    } else {
        b
    }
}

fn csharp_format_f4(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0.0000".to_string()
        } else {
            "0.0000".to_string()
        };
    }
    // C# "{0:F4}" rounds half-away-from-zero (for large values Rust's {:.4} matches exact IEEE decimal;
    // for 0.5/10000 ties, scale and round away from zero when within safe f64 range).
    let abs = x.abs();
    if abs < 1e11 {
        let scaled = (abs * 10_000.0).round() as u64;
        let int_part = scaled / 10_000;
        let frac_part = scaled % 10_000;
        let sign = if x.is_sign_negative() { "-" } else { "" };
        format!("{sign}{int_part}.{frac_part:04}")
    } else {
        format!("{x:.4}")
    }
}

fn csharp_format_zero(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }
    let abs = x.abs();
    if abs < 1e15 {
        let r = x.round();
        if r == 0.0 {
            return if x.is_sign_negative() {
                "-0".to_string()
            } else {
                "0".to_string()
            };
        }
        format!("{r:.0}")
    } else {
        // C# custom format "0" limits precision to 15 significant digits for double.
        let sign = if x.is_sign_negative() { "-" } else { "" };
        let sci = format!("{abs:.14e}");
        if let Some((mantissa, exp_str)) = sci.split_once('e') {
            let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
            let exp: usize = exp_str.parse().unwrap_or(0);
            if exp + 1 >= digits.len() {
                let zeros = exp + 1 - digits.len();
                return format!("{sign}{digits}{}", "0".repeat(zeros));
            }
        }
        format!("{x:.0}")
    }
}

/// Port of `WeatherIconName` (`Weather/WeatherLogic.cs:8`).
pub fn weather_icon_name(weather_code: i64, is_day: bool) -> Result<String, String> {
    if !is_day {
        return Ok("Night".to_string());
    }
    let name = match weather_code as i32 {
        0 | 1 => "Clear",
        2 => "PartlyClear",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 | 56 | 57 => "Drizzle",
        61 | 63 | 66 => "Rain",
        65 | 67 | 80 | 81 | 82 => "HeavyRain",
        71 | 73 => "Snow",
        75 | 77 | 85 | 86 => "HeavySnow",
        95 | 96 | 99 => "Thunderstorm",
        _ => "None",
    };
    Ok(name.to_string())
}

/// Port of `ForecastLayoutTier` (`Weather/WeatherLogic.cs:30`).
pub fn forecast_layout_tier(width: f64, height: f64) -> Result<String, String> {
    let tier = if width > 230.0 {
        if height > 230.0 {
            "Large"
        } else if height > 150.0 {
            "Wide"
        } else {
            "Tiny"
        }
    } else if width > 150.0 && height > 150.0 {
        "Small"
    } else {
        "Tiny"
    };
    Ok(tier.to_string())
}

/// Port of `MetricDisplayValue` (`Weather/WeatherLogic.cs:51`).
pub fn metric_display_value(value: f64) -> Result<i64, String> {
    Ok(i64::from(value.round_ties_even() as i32))
}

/// Port of `MetricFontSize` (`Weather/WeatherLogic.cs:56`).
pub fn metric_font_size(value: f64) -> Result<i64, String> {
    let rounded = value.round_ties_even() as i32;
    let text = rounded.to_string();
    let len = text.len() as i32;
    if len <= 2 {
        Ok(50)
    } else {
        Ok(i64::from(100 / len))
    }
}

/// Port of `MetricStrokeDashOffset` (`Weather/WeatherLogic.cs:67`).
pub fn metric_stroke_dash_offset(min: f64, max: f64, value: f64) -> Result<f64, String> {
    let span = max - min;
    if span == 0.0 {
        return Ok(0.0);
    }
    if min > max {
        return Err(format!(
            "System.ArgumentException: '{min}' cannot be greater than {max}."
        ));
    }
    let clamped = if value.is_nan() || min.is_nan() || max.is_nan() {
        value
    } else if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    };
    Ok((clamped - min) / span * 21.0)
}

/// Port of `DailyGraphWeights` (`Weather/WeatherLogic.cs:78`).
pub fn daily_graph_weights(
    day_min: f64,
    day_max: f64,
    global_min: f64,
    global_max: f64,
) -> Result<Vec<f64>, String> {
    Ok(vec![
        day_min - global_min,
        day_max - day_min,
        global_max - day_max,
    ])
}

/// Port of `MetricMargin` (`Weather/WeatherLogic.cs:88`).
pub fn metric_margin(width: f64, height: f64) -> Result<f64, String> {
    let m = csharp_min(width, height);
    Ok(if m >= 150.0 { m * 0.1 } else { 6.0 })
}

/// Port of `NextSunEventKind` (`Weather/WeatherLogic.cs:94`).
pub fn next_sun_event_kind(
    now_unix: i64,
    today_sunrise_unix: i64,
    today_sunset_unix: i64,
) -> Result<String, String> {
    let kind = if now_unix < today_sunrise_unix {
        "SunriseToday"
    } else if now_unix < today_sunset_unix {
        "SunsetToday"
    } else {
        "SunriseTomorrow"
    };
    Ok(kind.to_string())
}

/// Port of `BuildForecastUrl` (`Weather/WeatherLogic.cs:107`).
pub fn build_forecast_url(
    latitude: f64,
    longitude: f64,
    temperature_unit: String,
) -> Result<String, String> {
    let lat = csharp_format_f4(latitude);
    let lon = csharp_format_f4(longitude);
    Ok(format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&temperature_unit={temperature_unit}&current=temperature_2m,weathercode,surface_pressure&hourly=temperature_2m,weathercode,uv_index,is_day&daily=temperature_2m_min,temperature_2m_max,weathercode,sunrise,sunset&timezone=auto"
    ))
}

/// Port of `BuildAirQualityUrl` (`Weather/WeatherLogic.cs:114`).
pub fn build_air_quality_url(latitude: f64, longitude: f64) -> Result<String, String> {
    let lat = csharp_format_f4(latitude);
    let lon = csharp_format_f4(longitude);
    Ok(format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=european_aqi"
    ))
}

/// Port of `BuildGeocodingUrl` (`Weather/WeatherLogic.cs:121`).
pub fn build_geocoding_url(city_name: String, two_letter_lang: String) -> Result<String, String> {
    Ok(format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={city_name}&language={two_letter_lang}"
    ))
}

/// Port of `FormatTemperature` (`Weather/WeatherLogic.cs:126`).
pub fn format_temperature(temp: f64) -> Result<String, String> {
    Ok(format!("{}\u{00B0}", csharp_format_zero(temp)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_core_parity() {
        assert_eq!(weather_icon_name(0, true).ok().as_deref(), Some("Clear"));
        assert_eq!(weather_icon_name(0, false).ok().as_deref(), Some("Night"));
        assert_eq!(
            weather_icon_name(99, true).ok().as_deref(),
            Some("Thunderstorm")
        );
        assert_eq!(
            forecast_layout_tier(336.0, 160.0).ok().as_deref(),
            Some("Wide")
        );
        assert_eq!(
            forecast_layout_tier(336.0, 336.0).ok().as_deref(),
            Some("Large")
        );
        assert_eq!(metric_font_size(25.0).ok(), Some(50));
        assert_eq!(metric_font_size(1013.0).ok(), Some(25));
        assert!(metric_stroke_dash_offset(10.0, 5.0, 7.0).is_err());
        assert_eq!(format_temperature(-2.5).ok().as_deref(), Some("-3\u{00B0}"));
    }
}
