use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, Timelike};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};

use crate::i18n::{tr, weekday_abbrev};
use crate::icons::weather_svg;
use crate::models::{ForecastModel, WidgetLayout};
use crate::platform::graphics::{
    draw_arc, draw_svg_icon, draw_text, draw_text_shadowed, fill_round_rect, rect, rgba,
};
use crate::weather_client::{
    CurrentForecast, DailyForecast, ForecastResponse, HourlyForecast, WeatherCacheEntry,
};
use crate::weather_rs;
use crate::widgets::WidgetRenderCtx;

fn preview_forecast() -> ForecastResponse {
    ForecastResponse {
        current: Some(CurrentForecast {
            temperature_2m: 23.0,
            weathercode: 1,
            surface_pressure: 1014.0,
            apparent_temperature: Some(24.0),
            relative_humidity_2m: Some(58.0),
            wind_speed_10m: Some(11.0),
        }),
        hourly: Some(HourlyForecast {
            temperature_2m: vec![
                18.0, 17.0, 17.0, 16.0, 16.0, 17.0, 18.0, 19.0, 21.0, 22.0, 23.0, 24.0, 25.0, 25.0,
                24.0, 23.0, 22.0, 21.0, 20.0, 19.0, 19.0, 18.0, 18.0, 17.0,
            ],
            uv_index: vec![
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.8, 3.5, 5.2, 6.4, 7.1, 7.3, 6.2, 4.8, 3.1,
                1.5, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            weathercode: vec![1; 24],
            is_day: vec![
                0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
            ],
        }),
        daily: Some(DailyForecast {
            time: vec![
                "2026-11-23".into(),
                "2026-11-24".into(),
                "2026-11-25".into(),
                "2026-11-26".into(),
                "2026-11-27".into(),
                "2026-11-28".into(),
                "2026-11-29".into(),
            ],
            temperature_2m_min: vec![16.0, 15.0, 14.0, 15.0, 16.0, 17.0, 16.0],
            temperature_2m_max: vec![25.0, 24.0, 22.0, 23.0, 26.0, 27.0, 25.0],
            weathercode: vec![1, 2, 3, 61, 2, 0, 1],
            sunrise: vec!["2026-11-23T06:24".into(), "2026-11-24T06:25".into()],
            sunset: vec!["2026-11-23T17:14".into(), "2026-11-24T17:14".into()],
        }),
    }
}

fn fetch_data(
    ctx: &WidgetRenderCtx<'_>,
    model: &ForecastModel,
) -> (Option<ForecastResponse>, Option<f64>) {
    if ctx.is_preview {
        return (Some(preview_forecast()), Some(28.0));
    }
    let ttl_secs = (ctx.weather_cfg.refresh_minutes.clamp(5, 180) as u64) * 60;
    let entry: Option<WeatherCacheEntry> =
        ctx.weather.get_or_fetch(model, ctx.lang, ttl_secs, false);
    match entry {
        Some(e) => (e.forecast, e.aqi),
        None => (None, None),
    }
}

fn format_wind(kmh: f64, wind_unit: &str) -> String {
    match wind_unit {
        "mph" => format!("{} mph", (kmh * 0.621_371).round() as i32),
        "ms" => format!("{:.1} m/s", kmh / 3.6),
        _ => format!("{} km/h", kmh.round() as i32),
    }
}

fn format_hour_label(hr: usize, use_24h: bool) -> String {
    if use_24h {
        format!("{hr:02}")
    } else {
        let h12 = match hr % 12 {
            0 => 12,
            v => v,
        };
        let suf = if hr < 12 { "a" } else { "p" };
        format!("{h12}{suf}")
    }
}

fn format_extra_metrics(fc: &ForecastResponse, cfg: &crate::models::WeatherConfig) -> String {
    let mut parts = Vec::new();
    if let Some(cur) = &fc.current {
        if cfg.show_feels_like
            && let Some(feels) = cur.apparent_temperature
        {
            parts.push(format!("Feels {}", fmt_temp(feels)));
        }
        if cfg.show_humidity_wind {
            if let Some(rh) = cur.relative_humidity_2m {
                parts.push(format!("{}%", rh.round() as i32));
            }
            if let Some(ws) = cur.wind_speed_10m {
                parts.push(format_wind(ws, &cfg.wind_unit));
            }
        }
    }
    parts.join(" · ")
}

fn fmt_temp(t: f64) -> String {
    weather_rs::port::format_temperature(t).unwrap_or_else(|_| format!("{t:.0}°"))
}

fn weather_desc<'a>(lang: &str, code: i32) -> &'a str {
    let key: &'static str = match code {
        0 => "Weather_Code_0",
        1 => "Weather_Code_1",
        2 => "Weather_Code_2",
        3 => "Weather_Code_3",
        45 => "Weather_Code_45",
        48 => "Weather_Code_48",
        51 => "Weather_Code_51",
        53 => "Weather_Code_53",
        55 => "Weather_Code_55",
        56 => "Weather_Code_56",
        57 => "Weather_Code_57",
        61 => "Weather_Code_61",
        63 => "Weather_Code_63",
        65 => "Weather_Code_65",
        66 => "Weather_Code_66",
        67 => "Weather_Code_67",
        71 => "Weather_Code_71",
        73 => "Weather_Code_73",
        75 => "Weather_Code_75",
        77 => "Weather_Code_77",
        80 => "Weather_Code_80",
        81 => "Weather_Code_81",
        82 => "Weather_Code_82",
        85 => "Weather_Code_85",
        86 => "Weather_Code_86",
        95 => "Weather_Code_95",
        96 => "Weather_Code_96",
        99 => "Weather_Code_99",
        _ => "Weather_Code_0",
    };
    tr(lang, key)
}

fn is_currently_day(forecast: &ForecastResponse) -> bool {
    let hour = Local::now().hour() as usize;
    forecast
        .hourly
        .as_ref()
        .and_then(|h| h.is_day.get(hour))
        .is_none_or(|&d| d != 0)
}

/// `Weather / Forecast` — Responsive (`Tiny`, `Small`, `Wide`, `Large`) full weather widget.
pub fn render_forecast(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (forecast_opt, _) = fetch_data(ctx, &model);
    let tier = weather_rs::port::forecast_layout_tier(f64::from(ctx.width), f64::from(ctx.height))
        .unwrap_or_else(|_| "Small".to_string());

    let w = ctx.width;
    let h = ctx.height;
    let pad = 13.0_f32;

    let Some(fc) = forecast_opt else {
        draw_text(
            ctx.gfx,
            ctx.dc,
            &model.name,
            rect(pad, pad, w - pad, pad + 22.0),
            ctx.font_family,
            14.0,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            false,
        );
        draw_text(
            ctx.gfx,
            ctx.dc,
            "Loading…",
            rect(pad, pad + 24.0, w - pad, h - pad),
            ctx.font_family,
            13.0,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.8),
            false,
        );
        return;
    };

    let current_temp = fc.current.as_ref().map_or(0.0, |c| c.temperature_2m);
    let current_code = fc.current.as_ref().map_or(0, |c| c.weathercode);
    let is_day = is_currently_day(&fc);
    let icon_name = weather_rs::port::weather_icon_name(i64::from(current_code), is_day)
        .unwrap_or_else(|_| "Clear".to_string());
    let temp_str = fmt_temp(current_temp);
    let desc_str = weather_desc(ctx.lang, current_code);

    let (day_min, day_max) = fc
        .daily
        .as_ref()
        .map(|d| {
            (
                d.temperature_2m_min
                    .first()
                    .copied()
                    .unwrap_or(current_temp),
                d.temperature_2m_max
                    .first()
                    .copied()
                    .unwrap_or(current_temp),
            )
        })
        .unwrap_or((current_temp, current_temp));
    let hilo = format!("H:{}  L:{}", fmt_temp(day_max), fmt_temp(day_min));
    let extra = format_extra_metrics(&fc, ctx.weather_cfg);
    let hourly_slots = (ctx.weather_cfg.hourly_count.clamp(4, 8)) as usize;
    let show_cond = ctx.weather_cfg.show_condition;
    let show_hilo = ctx.weather_cfg.show_high_low;

    match tier.as_str() {
        "Tiny" => {
            let shadow = ctx.text_shadow();
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &model.name,
                rect(pad, 6.0, w - pad, 22.0),
                ctx.font_family,
                11.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(0.98),
                shadow,
                false,
            );
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &temp_str,
                rect(pad, 20.0, w * 0.65, h - 6.0),
                ctx.font_family,
                (h * 0.44).clamp(18.0, 36.0),
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_svg_icon(
                ctx.gfx,
                ctx.dc,
                weather_svg(&icon_name),
                rect(w - pad - 28.0, h * 0.5 - 12.0, w - pad, h * 0.5 + 16.0),
                ctx.accent(1.0),
            );
        }
        "Small" => {
            let shadow = ctx.text_shadow();
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &model.name,
                rect(pad, pad, w - pad, pad + 20.0),
                ctx.font_family,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &temp_str,
                rect(pad, pad + 18.0, w - pad, pad + 68.0),
                ctx.font_family,
                38.0,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_svg_icon(
                ctx.gfx,
                ctx.dc,
                weather_svg(&icon_name),
                rect(pad, h - pad - 38.0, pad + 20.0, h - pad - 18.0),
                ctx.accent(1.0),
            );
            if show_cond {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    desc_str,
                    rect(pad + 24.0, h - pad - 39.0, w - pad, h - pad - 18.0),
                    ctx.font_family,
                    11.5,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.fg(0.95),
                    shadow,
                    false,
                );
            }
            let bottom_line = if !extra.is_empty() && !show_hilo {
                extra.as_str()
            } else if show_hilo {
                hilo.as_str()
            } else {
                ""
            };
            if !bottom_line.is_empty() {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    bottom_line,
                    rect(pad, h - pad - 17.0, w - pad, h - pad + 2.0),
                    ctx.font_family,
                    11.0,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.95),
                    shadow,
                    false,
                );
            }
        }
        "Wide" => {
            let shadow = ctx.text_shadow();
            // Top header: city + current temp on left; icon + condition + H/L/extra on right
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &model.name,
                rect(pad, pad - 2.0, w * 0.66, pad + 18.0),
                ctx.font_family,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &temp_str,
                rect(pad, pad + 14.0, w * 0.42, pad + 60.0),
                ctx.font_family,
                36.0,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_svg_icon(
                ctx.gfx,
                ctx.dc,
                weather_svg(&icon_name),
                rect(w - pad - 22.0, pad, w - pad, pad + 22.0),
                ctx.accent(1.0),
            );
            let right_line_1 = if show_cond && !extra.is_empty() && !show_hilo {
                format!("{desc_str} · {extra}")
            } else if show_cond {
                desc_str.to_string()
            } else if !extra.is_empty() {
                extra.clone()
            } else {
                String::new()
            };
            if !right_line_1.is_empty() {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    &right_line_1,
                    rect(w * 0.36, pad + 24.0, w - pad, pad + 41.0),
                    ctx.font_family,
                    11.5,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.fg(0.95),
                    shadow,
                    false,
                );
            }
            let compact_extra = if w < 275.0 && show_hilo {
                // Keep narrow Wide/preview cards crisp without clipping into the large temp
                extra.replace("Feels ", "FL ")
            } else {
                extra.clone()
            };
            let right_line_2 = match (show_hilo, !compact_extra.is_empty() && show_cond) {
                (true, true) if w >= 285.0 => format!("{hilo} · {compact_extra}"),
                (true, true) => hilo.clone(),
                (true, false) => hilo.clone(),
                (false, true) => compact_extra,
                (false, false) => String::new(),
            };
            if !right_line_2.is_empty() {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    &right_line_2,
                    rect(w * 0.34, pad + 41.0, w - pad, pad + 58.0),
                    ctx.font_family,
                    if right_line_2.len() > 26 { 9.5 } else { 10.5 },
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.95),
                    shadow,
                    false,
                );
            }

            // Hourly strip (user-configurable 4..8 upcoming hours)
            draw_hourly_strip(
                ctx,
                &fc,
                rect(pad, pad + 64.0, w - pad, h - pad + 2.0),
                hourly_slots,
            );
        }
        _ => {
            let shadow = ctx.text_shadow();
            // Large: Top header + configurable hourly columns + 5..7 daily forecast rows with range bars
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &model.name,
                rect(pad, pad - 2.0, w * 0.66, pad + 18.0),
                ctx.font_family,
                14.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_text_shadowed(
                ctx.gfx,
                ctx.dc,
                &temp_str,
                rect(pad, pad + 16.0, w * 0.44, pad + 62.0),
                ctx.font_family,
                38.0,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                shadow,
                false,
            );
            draw_svg_icon(
                ctx.gfx,
                ctx.dc,
                weather_svg(&icon_name),
                rect(w - pad - 24.0, pad, w - pad, pad + 24.0),
                ctx.accent(1.0),
            );
            if show_cond {
                draw_text(
                    ctx.gfx,
                    ctx.dc,
                    desc_str,
                    rect(w * 0.42, pad + 26.0, w - pad, pad + 44.0),
                    ctx.font_family,
                    12.0,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.fg(0.92),
                    false,
                );
            }
            let sub_line = match (show_hilo, !extra.is_empty()) {
                (true, true) => format!("{hilo} · {extra}"),
                (true, false) => hilo.clone(),
                (false, true) => extra.clone(),
                (false, false) => String::new(),
            };
            if !sub_line.is_empty() {
                draw_text(
                    ctx.gfx,
                    ctx.dc,
                    &sub_line,
                    rect(w * 0.32, pad + 44.0, w - pad, pad + 60.0),
                    ctx.font_family,
                    11.0,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                    DWRITE_TEXT_ALIGNMENT_TRAILING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.9),
                    false,
                );
            }

            let hourly_top = pad + 66.0;
            let hourly_h = 66.0;
            fill_round_rect(
                ctx.dc,
                rect(pad, hourly_top - 3.0, w - pad, hourly_top - 2.0),
                0.5,
                ctx.fg(0.12),
            );
            draw_hourly_strip(
                ctx,
                &fc,
                rect(pad, hourly_top, w - pad, hourly_top + hourly_h),
                hourly_slots,
            );

            let daily_top = hourly_top + hourly_h + 6.0;
            fill_round_rect(
                ctx.dc,
                rect(pad, daily_top - 4.0, w - pad, daily_top - 3.0),
                0.5,
                ctx.fg(0.12),
            );
            draw_daily_rows(ctx, &fc, rect(pad, daily_top, w - pad, h - pad));
        }
    }
}

fn draw_hourly_strip(
    ctx: &WidgetRenderCtx<'_>,
    fc: &ForecastResponse,
    area: windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F,
    count: usize,
) {
    let Some(hourly) = &fc.hourly else { return };
    let now_h = Local::now().hour() as usize;
    let col_w = (area.right - area.left) / count as f32;
    let h = area.bottom - area.top;

    for i in 0..count {
        let idx = (now_h + i + 1).min(hourly.temperature_2m.len().saturating_sub(1));
        let hr = (now_h + i + 1) % 24;
        let x0 = area.left + i as f32 * col_w;
        let temp = hourly.temperature_2m.get(idx).copied().unwrap_or(0.0);
        let code = hourly.weathercode.get(idx).copied().unwrap_or(0);
        let day_flag = hourly.is_day.get(idx).copied().unwrap_or(1) != 0;
        let icon = weather_rs::port::weather_icon_name(i64::from(code), day_flag)
            .unwrap_or_else(|_| "Clear".to_string());

        draw_text(
            ctx.gfx,
            ctx.dc,
            &format_hour_label(hr, ctx.weather_cfg.use_24_hour_clock),
            rect(x0, area.top, x0 + col_w, area.top + h * 0.30),
            ctx.font_family,
            10.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.9),
            false,
        );
        let icon_s = (h * 0.28).clamp(12.0, 20.0);
        let cx = x0 + col_w * 0.5;
        let cy = area.top + h * 0.50;
        draw_svg_icon(
            ctx.gfx,
            ctx.dc,
            weather_svg(&icon),
            rect(
                cx - icon_s * 0.5,
                cy - icon_s * 0.5,
                cx + icon_s * 0.5,
                cy + icon_s * 0.5,
            ),
            ctx.accent(0.95),
        );
        draw_text(
            ctx.gfx,
            ctx.dc,
            &fmt_temp(temp),
            rect(x0, area.top + h * 0.68, x0 + col_w, area.bottom),
            ctx.font_family,
            11.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            false,
        );
    }
}

fn draw_daily_rows(
    ctx: &WidgetRenderCtx<'_>,
    fc: &ForecastResponse,
    area: windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F,
) {
    let Some(daily) = &fc.daily else { return };
    let n = daily
        .time
        .len()
        .min(daily.temperature_2m_min.len())
        .min(daily.temperature_2m_max.len())
        .min(7);
    if n == 0 {
        return;
    }

    let global_min = daily.temperature_2m_min[..n]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let global_max = daily.temperature_2m_max[..n]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    let avail_h = (area.bottom - area.top).max(20.0);
    let visible = ((avail_h / 22.0).floor() as usize).clamp(1, n);
    let row_h = avail_h / visible as f32;
    let total_w = area.right - area.left;

    for i in 0..visible {
        let y0 = area.top + i as f32 * row_h;
        let day_label = if i == 0 {
            tr(ctx.lang, "Clock_Today").to_string()
        } else if let Ok(d) = NaiveDate::parse_from_str(&daily.time[i], "%Y-%m-%d") {
            weekday_abbrev(ctx.lang, d.weekday().num_days_from_sunday() as usize).to_string()
        } else {
            daily.time[i].clone()
        };

        let dmin = daily.temperature_2m_min[i];
        let dmax = daily.temperature_2m_max[i];
        let code = daily.weathercode.get(i).copied().unwrap_or(0);
        let icon = weather_rs::port::weather_icon_name(i64::from(code), true)
            .unwrap_or_else(|_| "Clear".to_string());

        let day_w = (total_w * 0.22).clamp(36.0, 62.0);
        let icon_x = area.left + day_w;
        let min_x = icon_x + 24.0;
        let min_w = 30.0;
        let max_w = 30.0;
        let bar_x = min_x + min_w + 4.0;
        let bar_r = (area.right - max_w - 4.0).max(bar_x + 12.0);

        draw_text(
            ctx.gfx,
            ctx.dc,
            &day_label,
            rect(area.left, y0, icon_x, y0 + row_h),
            ctx.font_family,
            11.5,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(0.95),
            false,
        );
        let icon_s = 14.0_f32;
        let cy = y0 + row_h * 0.5;
        draw_svg_icon(
            ctx.gfx,
            ctx.dc,
            weather_svg(&icon),
            rect(
                icon_x + 2.0,
                cy - icon_s * 0.5,
                icon_x + 2.0 + icon_s,
                cy + icon_s * 0.5,
            ),
            ctx.accent(0.95),
        );
        draw_text(
            ctx.gfx,
            ctx.dc,
            &fmt_temp(dmin),
            rect(min_x, y0, min_x + min_w, y0 + row_h),
            ctx.font_family,
            11.0,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.85),
            false,
        );

        // Temperature range bar using parity-gated `daily_graph_weights`
        let bar_h = 4.5_f32;
        fill_round_rect(
            ctx.dc,
            rect(bar_x, cy - bar_h * 0.5, bar_r, cy + bar_h * 0.5),
            bar_h * 0.5,
            ctx.fg(0.16),
        );
        let weights = weather_rs::port::daily_graph_weights(dmin, dmax, global_min, global_max)
            .unwrap_or_else(|_| vec![0.0, 1.0, 0.0]);
        let sum = weights.iter().sum::<f64>().max(0.001);
        let left_frac = (weights[0] / sum).clamp(0.0, 0.95) as f32;
        let max_mid = (1.0 - f64::from(left_frac)).max(0.05);
        let mid_frac = (weights[1] / sum).clamp(0.05, max_mid) as f32;
        let bw = bar_r - bar_x;
        fill_round_rect(
            ctx.dc,
            rect(
                bar_x + bw * left_frac,
                cy - bar_h * 0.5,
                bar_x + bw * (left_frac + mid_frac),
                cy + bar_h * 0.5,
            ),
            bar_h * 0.5,
            ctx.accent(1.0),
        );

        draw_text(
            ctx.gfx,
            ctx.dc,
            &fmt_temp(dmax),
            rect(bar_r + 4.0, y0, area.right, y0 + row_h),
            ctx.font_family,
            11.0,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            false,
        );
    }
}

/// Shared circular open-bottom arc gauge matching `Weather.MetricView` (`Temperature`, `UVIndex`, `Pressure`, `AirQuality`).
#[allow(clippy::too_many_arguments)]
fn render_metric_gauge(
    ctx: &WidgetRenderCtx<'_>,
    value: f64,
    min: f64,
    max: f64,
    icon_name: Option<&str>,
    show_minmax: bool,
    suffix: &str,
) {
    let margin = weather_rs::port::metric_margin(f64::from(ctx.width), f64::from(ctx.height))
        .unwrap_or(10.0) as f32;
    let display_val = weather_rs::port::metric_display_value(value).unwrap_or(0);
    let font_base = weather_rs::port::metric_font_size(value).unwrap_or(50) as f32;
    let dash = weather_rs::port::metric_stroke_dash_offset(min.min(max), min.max(max), value)
        .unwrap_or(0.0);
    let frac = (dash / 21.0).clamp(0.0, 1.0) as f32;

    let d = ((ctx.width.min(ctx.height)) - margin * 2.0).max(24.0);
    let cx = ctx.width * 0.5;
    let cy = ctx.height * 0.5;
    let stroke = (d * 0.095).clamp(3.0, 16.0);
    let radius = ((d - stroke) * 0.5).max(10.0);

    // 250-degree open-bottom gauge arc: from 235° (bottom-left) clockwise by 250° to 125° (bottom-right)
    let start_deg = 235.0_f32;
    let total_sweep = 250.0_f32;
    draw_arc(
        ctx.gfx,
        ctx.dc,
        cx,
        cy,
        radius,
        start_deg,
        total_sweep,
        stroke,
        ctx.fg(0.18),
    );
    if frac > 0.01 {
        draw_arc(
            ctx.gfx,
            ctx.dc,
            cx,
            cy,
            radius,
            start_deg,
            total_sweep * frac,
            stroke,
            ctx.accent(1.0),
        );
    }

    // Center value text scaled proportionally to gauge diameter (`font_base` is authored for ~140px gauge)
    let val_size = (font_base * (d / 136.0)).clamp(12.0, 56.0);
    draw_text(
        ctx.gfx,
        ctx.dc,
        &format!("{display_val}{suffix}"),
        rect(
            cx - radius,
            cy - radius * 0.55,
            cx + radius,
            cy + radius * 0.45,
        ),
        ctx.font_family,
        val_size,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        ctx.fg(1.0),
        false,
    );

    // Bottom badge: either icon or min/max values
    let bottom_y = cy + radius * 0.62;
    if let Some(name) = icon_name {
        let isz = (d * 0.19).clamp(11.0, 26.0);
        draw_svg_icon(
            ctx.gfx,
            ctx.dc,
            weather_svg(name),
            rect(
                cx - isz * 0.5,
                bottom_y - isz * 0.5,
                cx + isz * 0.5,
                bottom_y + isz * 0.5,
            ),
            ctx.accent(1.0),
        );
    } else if show_minmax {
        let lbl_sz = (d * 0.105).clamp(9.0, 15.0);
        draw_text(
            ctx.gfx,
            ctx.dc,
            &format!("{}  {}", fmt_temp(min), fmt_temp(max)),
            rect(
                cx - radius,
                bottom_y - lbl_sz,
                cx + radius,
                bottom_y + lbl_sz,
            ),
            ctx.font_family,
            lbl_sz,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.95),
            false,
        );
    }
}

pub fn render_temperature(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (fc, _) = fetch_data(ctx, &model);
    let (cur, min, max) = if let Some(f) = fc {
        let cur = f.current.as_ref().map_or(0.0, |c| c.temperature_2m);
        let min = f
            .daily
            .as_ref()
            .and_then(|d| d.temperature_2m_min.first().copied())
            .unwrap_or(cur - 5.0);
        let max = f
            .daily
            .as_ref()
            .and_then(|d| d.temperature_2m_max.first().copied())
            .unwrap_or(cur + 5.0);
        (cur, min, max)
    } else {
        (0.0, 0.0, 1.0)
    };
    render_metric_gauge(ctx, cur, min, max, None, true, "°");
}

pub fn render_uv_index(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (fc, _) = fetch_data(ctx, &model);
    let hour = Local::now().hour() as usize;
    let uv = fc
        .and_then(|f| f.hourly)
        .and_then(|h| h.uv_index.get(hour).copied())
        .unwrap_or(0.0);
    render_metric_gauge(ctx, uv, 0.0, 10.0, Some("Clear"), false, "");
}

pub fn render_pressure(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (fc, _) = fetch_data(ctx, &model);
    let pressure = fc
        .and_then(|f| f.current)
        .map_or(1013.0, |c| c.surface_pressure);
    render_metric_gauge(ctx, pressure, 960.0, 1050.0, Some("Pressure"), false, "");
}

pub fn render_air_quality(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (_, aqi) = fetch_data(ctx, &model);
    render_metric_gauge(ctx, aqi.unwrap_or(0.0), 0.0, 100.0, Some("Wind"), false, "");
}

fn parse_iso_local_unix(s: &str, use_24h: bool) -> Option<(i64, String)> {
    let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok()?;
    let hhmm = if use_24h {
        format!("{:02}:{:02}", dt.hour(), dt.minute())
    } else {
        let h24 = dt.hour();
        let h12 = match h24 % 12 {
            0 => 12,
            v => v,
        };
        let suf = if h24 < 12 { "AM" } else { "PM" };
        format!("{h12}:{:02} {suf}", dt.minute())
    };
    Some((dt.and_utc().timestamp(), hhmm))
}

pub fn render_sunrise_sunset(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let model: ForecastModel = layout.parse_settings();
    let (fc, _) = fetch_data(ctx, &model);
    let use_24h = ctx.weather_cfg.use_24_hour_clock;

    let now_naive = Local::now().naive_local();
    let now_unix = now_naive.and_utc().timestamp();

    let (is_sunrise, time_str) = if let Some(daily) = fc.and_then(|f| f.daily) {
        let sr0 = daily
            .sunrise
            .first()
            .and_then(|s| parse_iso_local_unix(s, use_24h));
        let ss0 = daily
            .sunset
            .first()
            .and_then(|s| parse_iso_local_unix(s, use_24h));
        let sr1 = daily
            .sunrise
            .get(1)
            .and_then(|s| parse_iso_local_unix(s, use_24h));

        if let (Some((sr0_u, sr0_t)), Some((ss0_u, ss0_t))) = (sr0, ss0) {
            let kind = weather_rs::port::next_sun_event_kind(now_unix, sr0_u, ss0_u)
                .unwrap_or_else(|_| "SunriseToday".to_string());
            match kind.as_str() {
                "SunriseToday" => (true, sr0_t),
                "SunsetToday" => (false, ss0_t),
                _ => (true, sr1.map_or(sr0_t, |(_, t)| t)),
            }
        } else {
            (true, "--:--".to_string())
        }
    } else {
        (true, "--:--".to_string())
    };

    let m = ctx.width.min(ctx.height);
    let pad = (m * 0.12).clamp(8.0, 20.0);
    let icon_sz = (m * 0.34).clamp(20.0, 64.0);
    let cx = ctx.width * 0.5;
    let icon_y = ctx.height * 0.36;

    draw_svg_icon(
        ctx.gfx,
        ctx.dc,
        weather_svg(if is_sunrise { "Sunrise" } else { "Sunset" }),
        rect(
            cx - icon_sz * 0.5,
            icon_y - icon_sz * 0.5,
            cx + icon_sz * 0.5,
            icon_y + icon_sz * 0.5,
        ),
        ctx.accent(1.0),
    );

    let time_sz = (m * 0.22).clamp(14.0, 42.0);
    draw_text(
        ctx.gfx,
        ctx.dc,
        &time_str,
        rect(
            pad,
            ctx.height * 0.56,
            ctx.width - pad,
            ctx.height - pad * 0.5,
        ),
        ctx.font_family,
        time_sz,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        ctx.fg(1.0),
        false,
    );

    // Subtle divider horizon hint
    fill_round_rect(
        ctx.dc,
        rect(
            cx - icon_sz * 0.65,
            ctx.height * 0.54,
            cx + icon_sz * 0.65,
            ctx.height * 0.54 + 1.0,
        ),
        0.5,
        rgba(
            ctx.theme.fg_rgb.0,
            ctx.theme.fg_rgb.1,
            ctx.theme.fg_rgb.2,
            0.15,
        ),
    );
}
