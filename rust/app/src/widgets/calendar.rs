use chrono::{Datelike, Local, NaiveDate};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
};

use crate::calendar_rs;
use crate::i18n::{month_abbrev, month_full, weekday_abbrev, weekday_shortest};
use crate::models::{MonthCalendarModel, WidgetLayout};
use crate::platform::graphics::{draw_text_shadowed, fill_circle, rect, rgba};
use crate::widgets::WidgetRenderCtx;

/// `Calendar / Date` — Abbreviated weekday + accent month on top, bold Apple-style day number below.
pub fn render_date(ctx: &mut WidgetRenderCtx<'_>) {
    let now = Local::now();
    let w = ctx.width;
    let h = ctx.height;
    let m = w.min(h);
    let pad = (m * 0.10).clamp(8.0, 20.0);
    let shadow = ctx.text_shadow();

    let month_raw = month_abbrev(ctx.lang, now.month());
    let dow_raw = weekday_abbrev(ctx.lang, now.weekday().num_days_from_sunday() as usize);
    let month = calendar_rs::port::strip_trailing_dot(month_raw.to_string())
        .unwrap_or_else(|_| month_raw.to_string());
    let dow = calendar_rs::port::strip_trailing_dot(dow_raw.to_string())
        .unwrap_or_else(|_| dow_raw.to_string());
    let day = now.day().to_string();

    let header_size = (m * 0.155).clamp(11.0, 32.0);
    let header_h = header_size * 1.45;
    let header_y = pad;

    // Centered header line: "Mon" (accent bold) + " " + "Nov" (semibold foreground)
    let half_w = (w - pad * 2.0) * 0.5;
    draw_text_shadowed(
        ctx.gfx,
        ctx.dc,
        &dow,
        rect(pad, header_y, pad + half_w - 2.0, header_y + header_h),
        ctx.font_family,
        header_size,
        DWRITE_FONT_WEIGHT_BOLD,
        windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_ALIGNMENT_TRAILING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        ctx.accent(1.0),
        shadow,
        false,
    );
    draw_text_shadowed(
        ctx.gfx,
        ctx.dc,
        &month,
        rect(pad + half_w + 3.0, header_y, w - pad, header_y + header_h),
        ctx.font_family,
        header_size,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        ctx.fg(0.98),
        shadow,
        false,
    );

    // Large crisp SemiBold day number (Apple macOS/iOS Calendar weight — never thin Light!)
    let day_size = (m * 0.54).clamp(24.0, 140.0);
    draw_text_shadowed(
        ctx.gfx,
        ctx.dc,
        &day,
        rect(pad, header_y + header_h * 0.72, w - pad, h - pad * 0.6),
        ctx.font_family,
        day_size,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        ctx.fg(1.0),
        shadow,
        false,
    );
}

/// `Calendar / Month` — Uppercase full month header + 7-column localized month grid.
pub fn render_month(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: MonthCalendarModel = layout.parse_settings();
    let now = Local::now();
    let year = now.year();
    let month = now.month();
    let today = now.day();

    let w = ctx.width;
    let h = ctx.height;
    let pad_x = (w * 0.065).clamp(8.0, 22.0);
    let pad_y = (h * 0.08).clamp(8.0, 20.0);

    // Responsive sizing from parity-gated `calendar_rs`
    let base_text = calendar_rs::port::month_text_size(f64::from(w), f64::from(h))
        .unwrap_or(12.0)
        .clamp(8.0, 28.0) as f32;
    let margin_left =
        calendar_rs::port::month_margin_left(f64::from(w), f64::from(h)).unwrap_or(0.0) as f32;

    let month_title = month_full(ctx.lang, month).to_uppercase();
    let title_size = (base_text * 1.15).clamp(10.0, 26.0);
    let title_h = title_size * 1.35;
    let shadow = ctx.text_shadow();

    // Month header at top-left
    draw_text_shadowed(
        ctx.gfx,
        ctx.dc,
        &month_title,
        rect(
            (pad_x + margin_left.max(-pad_x * 0.35)).max(6.0),
            pad_y,
            w - pad_x,
            pad_y + title_h,
        ),
        ctx.font_family,
        title_size,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
        ctx.accent(1.0),
        shadow,
        false,
    );

    let first_of_month =
        NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_else(|| now.date_naive());
    let first_dow = i64::from(first_of_month.weekday().num_days_from_sunday());
    let first_day_pref = i64::from(settings.first_day_of_week.clamp(0, 6));

    let days_in_month =
        calendar_rs::port::days_in_month(i64::from(year), i64::from(month)).unwrap_or(30) as usize;
    let empty_days =
        calendar_rs::port::empty_days_count(first_dow, first_day_pref).unwrap_or(0) as usize;
    let weekday_order = calendar_rs::port::weekday_order(first_day_pref)
        .unwrap_or_else(|_| vec![1, 2, 3, 4, 5, 6, 0]);

    let total_cells = empty_days + days_in_month;
    let num_rows = total_cells.div_ceil(7).max(5);

    let grid_top = pad_y + title_h + 2.0;
    let grid_w = (w - pad_x * 2.0).max(20.0);
    let grid_h = (h - grid_top - pad_y * 0.75).max(20.0);
    let col_w = grid_w / 7.0;
    let row_h = grid_h / (num_rows + 1) as f32;
    let cell_font = (base_text * 0.84).min(row_h * 0.60).clamp(7.5, 21.0);

    // Row 0: weekday headers
    for (col, &dow) in weekday_order.iter().enumerate() {
        let label = weekday_shortest(ctx.lang, dow as usize);
        let is_wknd = calendar_rs::port::is_weekend(dow).unwrap_or(false);
        let col_color = if is_wknd {
            ctx.muted(0.75)
        } else {
            ctx.fg(0.95)
        };
        let x0 = pad_x + col as f32 * col_w;
        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            label,
            rect(x0, grid_top, x0 + col_w, grid_top + row_h),
            ctx.font_family,
            cell_font * 0.92,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            col_color,
            shadow,
            false,
        );
    }

    // Days 1..=days_in_month
    for day in 1..=days_in_month {
        let cell_idx = empty_days + (day - 1);
        let row = cell_idx / 7;
        let col = cell_idx % 7;
        let x0 = pad_x + col as f32 * col_w;
        let y0 = grid_top + (row + 1) as f32 * row_h;
        let dow = weekday_order.get(col).copied().unwrap_or(1);
        let is_wknd = calendar_rs::port::is_weekend(dow).unwrap_or(false);
        let is_today = day as u32 == today;

        if is_today {
            let cx = x0 + col_w * 0.5;
            let cy = y0 + row_h * 0.5;
            let r = (row_h * 0.52).min(col_w * 0.44).max(6.0);
            fill_circle(ctx.dc, cx, cy, r, ctx.accent(1.0));
        }

        let txt_color = if is_today {
            if ctx.theme.monochrome && !ctx.theme.is_dark {
                rgba(1.0, 1.0, 1.0, 1.0)
            } else if ctx.theme.monochrome && ctx.theme.is_dark {
                rgba(0.1, 0.1, 0.1, 1.0)
            } else {
                rgba(1.0, 1.0, 1.0, 1.0)
            }
        } else if is_wknd {
            ctx.muted(0.75)
        } else {
            ctx.fg(0.98)
        };

        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            &day.to_string(),
            rect(x0, y0, x0 + col_w, y0 + row_h),
            ctx.font_family,
            cell_font,
            if is_today {
                DWRITE_FONT_WEIGHT_BOLD
            } else {
                DWRITE_FONT_WEIGHT_SEMI_BOLD
            },
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            txt_color,
            if is_today { None } else { shadow },
            false,
        );
    }
}
