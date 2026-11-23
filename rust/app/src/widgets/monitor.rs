use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};

use crate::icons::monitor_svg;
use crate::models::{SingleMetricModel, WidgetLayout};
use crate::monitor_rs;
use crate::platform::graphics::{draw_arc, draw_circle, draw_svg_icon, draw_text, rect};
use crate::widgets::WidgetRenderCtx;

/// `Monitor / SingleMetric` — Circular progress ring + centered metric SVG icon + percentage text.
pub fn render_single_metric(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: SingleMetricModel = layout.parse_settings();
    let val = if ctx.is_preview {
        match settings.metric {
            0 => 0.36,
            1 => 0.62,
            2 => 0.18,
            3 => 0.12,
            4 => 0.84,
            _ => 0.42,
        }
    } else {
        ctx.monitor.value(settings.metric).clamp(0.0, 1.0)
    };

    // Exact parity-gated math from `monitor_rs`
    let is_compact = monitor_rs::port::monitor_is_compact(f64::from(ctx.height)).unwrap_or(false);
    let margin = monitor_rs::port::monitor_margin(f64::from(ctx.height)).unwrap_or(12) as f32;
    let pct_text = monitor_rs::port::monitor_percent_text(val).unwrap_or_else(|_| "0%".to_string());
    let dash_offset = monitor_rs::port::monitor_stroke_dash_offset(val).unwrap_or(31.4);
    let progress = (1.0 - dash_offset / 31.4).clamp(0.0, 1.0) as f32;

    let avail_w = (ctx.width - margin * 2.0).max(20.0);
    let avail_h = (ctx.height - margin * 2.0).max(20.0);

    let (ring_cy, ring_diam, text_rect) = if is_compact {
        let d = avail_w.min(avail_h);
        (ctx.height * 0.5, d, None)
    } else {
        let text_h = (avail_h * 0.24).clamp(16.0, 34.0);
        let ring_space_h = (avail_h - text_h - 4.0).max(20.0);
        let d = avail_w.min(ring_space_h);
        let cy = margin + ring_space_h * 0.5;
        let tr = rect(
            margin,
            margin + ring_space_h + 2.0,
            ctx.width - margin,
            ctx.height - margin * 0.6,
        );
        (cy, d, Some(tr))
    };

    let cx = ctx.width * 0.5;
    let stroke = (ring_diam * (10.0 / 110.0)).clamp(3.0, 18.0);
    let radius = ((ring_diam - stroke) * 0.5).max(8.0);

    // Background track (opacity 0.2)
    draw_circle(ctx.dc, cx, ring_cy, radius, ctx.fg(0.20), stroke);

    // Foreground arc clockwise from top
    let sweep = progress * 360.0;
    if sweep > 0.5 {
        draw_arc(
            ctx.gfx,
            ctx.dc,
            cx,
            ring_cy,
            radius,
            0.0,
            sweep,
            stroke,
            ctx.accent(1.0),
        );
    }

    // Centered metric SVG icon
    let icon_size = (radius * 0.88).max(12.0);
    let icon_rect = rect(
        cx - icon_size * 0.5,
        ring_cy - icon_size * 0.5,
        cx + icon_size * 0.5,
        ring_cy + icon_size * 0.5,
    );
    draw_svg_icon(
        ctx.gfx,
        ctx.dc,
        monitor_svg(settings.metric),
        icon_rect,
        ctx.fg(0.95),
    );

    if let Some(tr) = text_rect {
        let font_sz = ((tr.bottom - tr.top) * 0.72).clamp(11.0, 26.0);
        draw_text(
            ctx.gfx,
            ctx.dc,
            &pct_text,
            tr,
            ctx.font_family,
            font_sz,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            false,
        );
    }
}
