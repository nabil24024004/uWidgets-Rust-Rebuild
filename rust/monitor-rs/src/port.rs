//! The Rust port of `Monitor/`.
//!
//! Faithful port of `Monitor/MonitorLogic.cs` verified against `.NET` via `rust-anything`.

#![forbid(unsafe_code)]

/// Port of `MonitorStrokeDashOffset` (`Monitor/MonitorLogic.cs:8`).
pub fn monitor_stroke_dash_offset(value: f64) -> Result<f64, String> {
    Ok(31.4 * (1.0 - value))
}

/// Port of `MonitorPercentText` (`Monitor/MonitorLogic.cs:13`).
pub fn monitor_percent_text(value: f64) -> Result<String, String> {
    let pct = (value * 100.0).round_ties_even() as i32;
    Ok(format!("{pct}%"))
}

/// Port of `MonitorIsCompact` (`Monitor/MonitorLogic.cs:19`).
pub fn monitor_is_compact(height: f64) -> Result<bool, String> {
    Ok(height < 100.0)
}

/// Port of `MonitorMargin` (`Monitor/MonitorLogic.cs:24`).
pub fn monitor_margin(height: f64) -> Result<i64, String> {
    Ok(if height < 100.0 { 6 } else { 12 })
}

/// Port of `ComputeRamRatio` (`Monitor/MonitorLogic.cs:29`).
pub fn compute_ram_ratio(total_kb: f64, free_kb: f64) -> Result<f64, String> {
    if total_kb <= 0.0 {
        return Ok(0.0);
    }
    Ok((total_kb - free_kb) / total_kb)
}

/// Port of `ComputeNetworkRatio` (`Monitor/MonitorLogic.cs:38`).
pub fn compute_network_ratio(
    sent_bytes_per_sec: f64,
    recv_bytes_per_sec: f64,
    link_speed_bits_per_sec: f64,
) -> Result<f64, String> {
    if link_speed_bits_per_sec <= 0.0 {
        return Ok(0.0);
    }
    Ok((sent_bytes_per_sec + recv_bytes_per_sec) * 8.0 / link_speed_bits_per_sec)
}

/// Port of `MetricLocaleKey` (`Monitor/MonitorLogic.cs:47`).
pub fn metric_locale_key(metric_type: i64) -> Result<String, String> {
    let key = match metric_type as i32 {
        0 => "Monitor_Metric_0",
        1 => "Monitor_Metric_1",
        2 => "Monitor_Metric_2",
        3 => "Monitor_Metric_3",
        4 => "Monitor_Metric_4",
        _ => "Monitor_Metric_0",
    };
    Ok(key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_math_parity() {
        assert!((monitor_stroke_dash_offset(0.5).unwrap_or(0.0) - 15.7).abs() < 1e-9);
        assert_eq!(monitor_percent_text(0.456).ok().as_deref(), Some("46%"));
        assert_eq!(monitor_is_compact(99.0).ok(), Some(true));
        assert_eq!(monitor_margin(100.0).ok(), Some(12));
        assert_eq!(compute_ram_ratio(16000.0, 4000.0).ok(), Some(0.75));
        assert_eq!(compute_network_ratio(125.0, 125.0, 2000.0).ok(), Some(1.0));
        assert_eq!(
            metric_locale_key(4).ok().as_deref(),
            Some("Monitor_Metric_4")
        );
    }
}
