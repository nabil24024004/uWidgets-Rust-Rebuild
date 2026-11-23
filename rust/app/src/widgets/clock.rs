use chrono::{Datelike, Local, NaiveDateTime, Timelike};
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Graphics::Direct2D::Common::{D2D_POINT_2F, D2D1_COLOR_F};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};

use crate::clock_rs;
use crate::i18n::{month_full, tr, weekday_full};
use crate::models::{ClockModel, WidgetLayout, WorldClockModel};
use crate::platform::graphics::{
    brush, draw_circle, draw_round_rect, draw_text, draw_text_shadowed, fill_circle,
    fill_round_rect, rect, rgba,
};
use crate::platform::system::{WinTimeZone, now_in_timezone};
use crate::widgets::{WidgetRenderCtx, rotate_around, scale_matrix, translate_matrix};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalogStyle {
    /// Minimalist circular frosted glass dial with 4 cardinal dots, sleek hands, crimson seconds hand.
    StyleI,
    /// Sculpted glass bezel ring + recessed center well, 60 ticks, bold 12/3/6/9, crimson seconds needle.
    StyleII,
    /// Squircle-contour tick ring hugging the glass perimeter, bold 12/3/6/9, Apple Watch orange seconds hand.
    StyleIII,
}

fn find_tz<'a>(zones: &'a [WinTimeZone], id: Option<&str>) -> Option<&'a WinTimeZone> {
    let target = id?;
    zones.iter().find(|z| z.id.eq_ignore_ascii_case(target))
}

/// Exact distance from center `(0, 0)` along ray `(sin(theta), -cos(theta))` to a rounded square
/// with half-size `half` and corner radius `rad`.
fn squircle_ray_dist(theta_rad: f32, half: f32, rad: f32) -> f32 {
    let ax = theta_rad.sin().abs();
    let ay = theta_rad.cos().abs();
    let c = (half - rad).max(0.0);

    if ax > 1e-5 {
        let tx = half / ax;
        if tx * ay <= c {
            return tx;
        }
    }
    if ay > 1e-5 {
        let ty = half / ay;
        if ty * ax <= c {
            return ty;
        }
    }
    // Ray hits the rounded corner arc centered at (c, c) with radius `rad`
    let b = c * (ax + ay);
    let disc = (b * b - (2.0 * c * c - rad * rad)).max(0.0);
    b + disc.sqrt()
}

fn draw_line_rounded(
    ctx: &WidgetRenderCtx<'_>,
    p0: D2D_POINT_2F,
    p1: D2D_POINT_2F,
    color: D2D1_COLOR_F,
    stroke: f32,
) {
    if let Ok(b) = brush(ctx.dc, color) {
        unsafe {
            ctx.dc.DrawLine(p0, p1, &b, stroke, &ctx.gfx.round_stroke);
        }
    }
}

/// Renders one of the 3 Liquid Glass analog clock designs in exact 1000×1000 logical coordinates.
#[allow(clippy::too_many_arguments)]
fn draw_analog_dial(
    ctx: &WidgetRenderCtx<'_>,
    cx: f32,
    cy: f32,
    diameter: f32,
    dt: NaiveDateTime,
    show_seconds: bool,
    style: AnalogStyle,
    abbrev: Option<&str>,
) {
    let scale = (diameter / 1000.0).max(0.01);
    let mut prev = Matrix3x2::identity();
    unsafe {
        ctx.dc.GetTransform(&mut prev);
        let xf = scale_matrix(scale, scale)
            * translate_matrix(cx - diameter * 0.5, cy - diameter * 0.5)
            * prev;
        ctx.dc.SetTransform(&xf);
    }

    let is_dark = ctx.theme.is_dark;
    let is_mono = ctx.theme.monochrome;
    let fg = ctx.fg(1.0);
    let shadow_col = if is_dark {
        rgba(0.0, 0.0, 0.0, 0.38)
    } else {
        rgba(0.0, 0.0, 0.0, 0.16)
    };

    // Signature watch seconds-hand colors (respecting Monochrome toggle)
    let crimson_red = if is_mono {
        ctx.fg(0.95)
    } else {
        rgba(1.0, 0.23, 0.19, 1.0) // Apple #FF3B30
    };
    let apple_orange = if is_mono {
        ctx.fg(0.95)
    } else {
        rgba(1.0, 0.58, 0.0, 1.0) // Apple Watch #FF9500
    };

    // Clock hand angles from parity-gated `clock_rs::port`
    let hour = i64::from(dt.hour());
    let minute = i64::from(dt.minute());
    let second = i64::from(dt.second());
    let millis = i64::from(dt.nanosecond() / 1_000_000);

    let h_deg = clock_rs::port::hours_angle(hour, minute).unwrap_or(0.0) as f32;
    let m_deg = clock_rs::port::minutes_angle(minute, second).unwrap_or(0.0) as f32;
    let s_deg = clock_rs::port::seconds_angle(second, millis).unwrap_or(0.0) as f32;

    let mut dial_xf = Matrix3x2::identity();
    unsafe { ctx.dc.GetTransform(&mut dial_xf) };

    match style {
        AnalogStyle::StyleI => {
            // === DESIGN 1: Minimal Circular Frosted Glass Lens Dial + 4 Cardinal Dots + Red Seconds ===
            // Raised translucent glass face disc inside the squircle frame
            let dial_fill = if is_dark {
                rgba(1.0, 1.0, 1.0, 0.11)
            } else {
                rgba(1.0, 1.0, 1.0, 0.72)
            };
            fill_circle(ctx.dc, 500.0, 500.0, 442.0, dial_fill);
            // Subtle inner top-lit glass rim bevel on the circular lens
            draw_circle(
                ctx.dc,
                500.0,
                500.0,
                441.0,
                if is_dark {
                    rgba(1.0, 1.0, 1.0, 0.26)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.85)
                },
                7.0,
            );
            draw_circle(ctx.dc, 500.0, 500.0, 434.0, rgba(0.0, 0.0, 0.0, 0.10), 4.0);

            // 4 ultra-clean cardinal dots (12, 3, 6, 9)
            let dot_col = if is_dark {
                rgba(1.0, 1.0, 1.0, 0.52)
            } else {
                rgba(0.35, 0.36, 0.40, 0.65)
            };
            for q in 0..4_u16 {
                let ang = (f32::from(q) * 90.0 - 90.0).to_radians();
                let dx = 500.0 + 356.0 * ang.cos();
                let dy = 500.0 + 356.0 * ang.sin();
                fill_circle(ctx.dc, dx, dy, 13.0, dot_col);
            }

            if let Some(code) = abbrev
                && !code.is_empty()
            {
                draw_text(
                    ctx.gfx,
                    ctx.dc,
                    code,
                    rect(280.0, 625.0, 720.0, 770.0),
                    ctx.font_family,
                    110.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.85),
                    false,
                );
            }

            // Sleek modern straight rounded baton hands (hour + minute) with subtle drop shadow
            let hand_col = if is_dark {
                rgba(0.97, 0.98, 1.0, 0.96)
            } else {
                rgba(0.08, 0.09, 0.11, 0.95)
            };

            // Hour hand
            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(h_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 508.0 },
                D2D_POINT_2F { x: 500.0, y: 278.0 },
                shadow_col,
                34.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 500.0 },
                D2D_POINT_2F { x: 500.0, y: 270.0 },
                hand_col,
                30.0,
            );

            // Minute hand
            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(m_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 508.0 },
                D2D_POINT_2F { x: 500.0, y: 178.0 },
                shadow_col,
                26.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 500.0 },
                D2D_POINT_2F { x: 500.0, y: 170.0 },
                hand_col,
                22.0,
            );

            unsafe { ctx.dc.SetTransform(&dial_xf) };
            fill_circle(ctx.dc, 500.0, 500.0, 30.0, hand_col);

            // Signature crimson seconds hand + red center hub
            if show_seconds {
                unsafe {
                    ctx.dc
                        .SetTransform(&(rotate_around(s_deg, 500.0, 500.0) * dial_xf));
                }
                draw_line_rounded(
                    ctx,
                    D2D_POINT_2F { x: 500.0, y: 500.0 },
                    D2D_POINT_2F { x: 500.0, y: 275.0 },
                    crimson_red,
                    16.0,
                );
                unsafe { ctx.dc.SetTransform(&dial_xf) };
                fill_circle(ctx.dc, 500.0, 500.0, 20.0, crimson_red);
            } else {
                fill_circle(ctx.dc, 500.0, 500.0, 15.0, crimson_red);
            }
        }
        AnalogStyle::StyleII => {
            // === DESIGN 2: Sculpted Glass Bezel Ring + Recessed Inner Well + 60 Ticks + 12/3/6/9 + Crimson Needle ===
            // Raised translucent sculpted circular glass bezel ring
            draw_circle(
                ctx.dc,
                500.0,
                500.0,
                444.0,
                if is_dark {
                    rgba(1.0, 1.0, 1.0, 0.16)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.78)
                },
                32.0,
            );
            // Outer bezel crisp catchlight & inner bezel depth shadow
            draw_circle(
                ctx.dc,
                500.0,
                500.0,
                460.0,
                if is_dark {
                    rgba(1.0, 1.0, 1.0, 0.30)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.90)
                },
                5.0,
            );
            draw_circle(
                ctx.dc,
                500.0,
                500.0,
                427.0,
                rgba(0.0, 0.0, 0.0, if is_dark { 0.28 } else { 0.12 }),
                7.0,
            );
            // Dial face interior + recessed center circular well
            fill_circle(
                ctx.dc,
                500.0,
                500.0,
                424.0,
                if is_dark {
                    rgba(1.0, 1.0, 1.0, 0.05)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.56)
                },
            );
            fill_circle(
                ctx.dc,
                500.0,
                500.0,
                215.0,
                if is_dark {
                    rgba(0.0, 0.0, 0.0, 0.14)
                } else {
                    rgba(0.0, 0.0, 0.0, 0.045)
                },
            );
            draw_circle(
                ctx.dc,
                500.0,
                500.0,
                215.0,
                if is_dark {
                    rgba(1.0, 1.0, 1.0, 0.09)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.60)
                },
                4.0,
            );

            // 60 crisp ticks inside the sculpted bezel
            for i in 0..60_u16 {
                let is_hour = i % 5 == 0;
                let ang = (f32::from(i) * 6.0 - 90.0).to_radians();
                let r_outer = 402.0_f32;
                let r_inner = if is_hour { 348.0 } else { 378.0 };
                let p0 = D2D_POINT_2F {
                    x: 500.0 + r_inner * ang.cos(),
                    y: 500.0 + r_inner * ang.sin(),
                };
                let p1 = D2D_POINT_2F {
                    x: 500.0 + r_outer * ang.cos(),
                    y: 500.0 + r_outer * ang.sin(),
                };
                let col = if is_hour { fg } else { ctx.fg(0.45) };
                draw_line_rounded(ctx, p0, p1, col, if is_hour { 18.0 } else { 8.0 });
            }

            // Bold cardinal numerals 12, 3, 6, 9
            let num_shadow = if is_dark {
                Some(rgba(0.0, 0.0, 0.0, 0.40))
            } else {
                None
            };
            for (lbl, x, y) in [
                ("12", 500.0, 266.0),
                ("3", 734.0, 500.0),
                ("6", 500.0, 734.0),
                ("9", 266.0, 500.0),
            ] {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    lbl,
                    rect(x - 95.0, y - 70.0, x + 95.0, y + 70.0),
                    ctx.font_family,
                    108.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    fg,
                    num_shadow,
                    false,
                );
            }

            if let Some(code) = abbrev
                && !code.is_empty()
            {
                draw_text(
                    ctx.gfx,
                    ctx.dc,
                    code,
                    rect(320.0, 570.0, 680.0, 665.0),
                    ctx.font_family,
                    78.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.85),
                    false,
                );
            }

            // Thick tactile Braun/Apple hands with soft drop shadows
            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(h_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 510.0 },
                D2D_POINT_2F { x: 500.0, y: 266.0 },
                shadow_col,
                42.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 500.0 },
                D2D_POINT_2F { x: 500.0, y: 256.0 },
                fg,
                36.0,
            );

            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(m_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 510.0 },
                D2D_POINT_2F { x: 500.0, y: 158.0 },
                shadow_col,
                34.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 500.0 },
                D2D_POINT_2F { x: 500.0, y: 148.0 },
                fg,
                28.0,
            );

            unsafe { ctx.dc.SetTransform(&dial_xf) };
            fill_circle(ctx.dc, 500.0, 500.0, 44.0, fg);

            // Long razor-sharp crimson-red seconds needle + short counterweight tail
            if show_seconds {
                unsafe {
                    ctx.dc
                        .SetTransform(&(rotate_around(s_deg, 500.0, 500.0) * dial_xf));
                }
                draw_line_rounded(
                    ctx,
                    D2D_POINT_2F { x: 500.0, y: 580.0 },
                    D2D_POINT_2F { x: 500.0, y: 118.0 },
                    crimson_red,
                    12.0,
                );
                unsafe { ctx.dc.SetTransform(&dial_xf) };
                fill_circle(ctx.dc, 500.0, 500.0, 22.0, crimson_red);
            } else {
                fill_circle(ctx.dc, 500.0, 500.0, 18.0, crimson_red);
            }
        }
        AnalogStyle::StyleIII => {
            // === DESIGN 3: Squircle Contour Liquid Glass Dial (Apple Watch / iOS 26 Style!) ===
            // 60 ticks follow the EXACT rounded-square squircle contour hugging the glass rim!
            let half = 456.0_f32;
            let corner_r = 152.0_f32;

            for i in 0..60_u16 {
                let is_hour = i % 5 == 0;
                let theta = (f32::from(i) * 6.0).to_radians();
                let ux = theta.sin();
                let uy = -theta.cos();
                let r_outer = squircle_ray_dist(theta, half, corner_r);
                let tick_len = if is_hour {
                    if i % 15 == 0 { 96.0 } else { 118.0 }
                } else {
                    50.0
                };
                let r_inner = (r_outer - tick_len).max(260.0);

                let p0 = D2D_POINT_2F {
                    x: 500.0 + r_inner * ux,
                    y: 500.0 + r_inner * uy,
                };
                let p1 = D2D_POINT_2F {
                    x: 500.0 + r_outer * ux,
                    y: 500.0 + r_outer * uy,
                };
                let col = if is_hour { ctx.fg(0.92) } else { ctx.fg(0.32) };
                draw_line_rounded(ctx, p0, p1, col, if is_hour { 19.0 } else { 11.0 });
            }

            // Bold rounded numerals at 12, 3, 6, 9 tucked cleanly inside the contour ticks
            let num_shadow = if is_dark {
                Some(rgba(0.0, 0.0, 0.0, 0.42))
            } else {
                None
            };
            for (lbl, x, y) in [
                ("12", 500.0, 252.0),
                ("3", 748.0, 500.0),
                ("6", 500.0, 748.0),
                ("9", 252.0, 500.0),
            ] {
                draw_text_shadowed(
                    ctx.gfx,
                    ctx.dc,
                    lbl,
                    rect(x - 105.0, y - 75.0, x + 105.0, y + 75.0),
                    ctx.font_family,
                    122.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.fg(0.92),
                    num_shadow,
                    false,
                );
            }

            if let Some(code) = abbrev
                && !code.is_empty()
            {
                draw_text(
                    ctx.gfx,
                    ctx.dc,
                    code,
                    rect(320.0, 575.0, 680.0, 665.0),
                    ctx.font_family,
                    78.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    ctx.muted(0.85),
                    false,
                );
            }

            // Signature Apple Watch baton hands (slender neck stem + thick rounded capsule paddle)
            // Hour hand
            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(h_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 468.0 },
                D2D_POINT_2F { x: 500.0, y: 416.0 },
                fg,
                18.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 422.0 },
                D2D_POINT_2F { x: 500.0, y: 242.0 },
                shadow_col,
                52.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 416.0 },
                D2D_POINT_2F { x: 500.0, y: 236.0 },
                fg,
                46.0,
            );

            // Minute hand
            unsafe {
                ctx.dc
                    .SetTransform(&(rotate_around(m_deg, 500.0, 500.0) * dial_xf));
            }
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 468.0 },
                D2D_POINT_2F { x: 500.0, y: 416.0 },
                fg,
                18.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 422.0 },
                D2D_POINT_2F { x: 500.0, y: 126.0 },
                shadow_col,
                44.0,
            );
            draw_line_rounded(
                ctx,
                D2D_POINT_2F { x: 500.0, y: 416.0 },
                D2D_POINT_2F { x: 500.0, y: 120.0 },
                fg,
                38.0,
            );

            unsafe { ctx.dc.SetTransform(&dial_xf) };
            fill_circle(ctx.dc, 500.0, 500.0, 38.0, fg);

            // Signature Apple Watch Orange (#FF9500) seconds hand + hollow center pin!
            if show_seconds {
                unsafe {
                    ctx.dc
                        .SetTransform(&(rotate_around(s_deg, 500.0, 500.0) * dial_xf));
                }
                draw_line_rounded(
                    ctx,
                    D2D_POINT_2F { x: 500.0, y: 580.0 },
                    D2D_POINT_2F { x: 500.0, y: 74.0 },
                    apple_orange,
                    18.0,
                );
                unsafe { ctx.dc.SetTransform(&dial_xf) };
                fill_circle(ctx.dc, 500.0, 500.0, 26.0, apple_orange);
                fill_circle(
                    ctx.dc,
                    500.0,
                    500.0,
                    10.0,
                    if is_dark {
                        rgba(0.10, 0.11, 0.14, 0.95)
                    } else {
                        rgba(1.0, 1.0, 1.0, 0.98)
                    },
                );
            } else {
                fill_circle(ctx.dc, 500.0, 500.0, 20.0, apple_orange);
                fill_circle(ctx.dc, 500.0, 500.0, 8.0, rgba(1.0, 1.0, 1.0, 0.95));
            }
        }
    }

    unsafe {
        ctx.dc.SetTransform(&prev);
    }
}

pub fn render_analog(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout, style: AnalogStyle) {
    let settings: ClockModel = layout.parse_settings();
    let tz = find_tz(ctx.timezones, settings.time_zone_id.as_deref());
    let dt = now_in_timezone(tz);

    let margin = if style == AnalogStyle::StyleIII {
        5.0_f32
    } else {
        8.0_f32
    };
    let diameter = (ctx.width.min(ctx.height) - margin * 2.0).max(20.0);
    draw_analog_dial(
        ctx,
        ctx.width * 0.5,
        ctx.height * 0.5,
        diameter,
        dt,
        settings.show_seconds,
        style,
        None,
    );
}

/// `Clock / Flip` — Liquid Glass Split-Flap Flip Clock (Design 4: `04 | 18` with metallic glass hinges!).
pub fn render_flip(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: ClockModel = layout.parse_settings();
    let tz = find_tz(ctx.timezones, settings.time_zone_id.as_deref());
    let dt = now_in_timezone(tz);

    let h24 = dt.hour();
    let (h_disp, ampm) = if settings.use_24_hours {
        (h24, None)
    } else {
        let h12 = if h24.is_multiple_of(12) { 12 } else { h24 % 12 };
        (h12, Some(if h24 < 12 { "AM" } else { "PM" }))
    };
    let hh_str = format!("{h_disp:02}");
    let mm_str = format!("{:02}", dt.minute());
    let ss_str = format!("{:02}", dt.second());

    let w = ctx.width;
    let h = ctx.height;
    let m = w.min(h);
    let is_dark = ctx.theme.is_dark;
    let pad_x = (m * 0.085).clamp(7.0, 20.0);
    let pad_y = (h * 0.14).clamp(10.0, 28.0);
    let show_three = settings.show_seconds && (w / h.max(1.0)) >= 1.65;

    // Outer recessed mechanical glass track tray
    let tray_r = rect(
        pad_x * 0.72,
        pad_y * 0.85,
        w - pad_x * 0.72,
        h - pad_y * 0.85,
    );
    let tray_rad = (m * 0.095).clamp(6.0, 18.0);
    fill_round_rect(
        ctx.dc,
        tray_r,
        tray_rad,
        if is_dark {
            rgba(0.0, 0.0, 0.0, 0.34)
        } else {
            rgba(0.0, 0.0, 0.0, 0.10)
        },
    );
    draw_round_rect(
        ctx.dc,
        tray_r,
        tray_rad,
        if is_dark {
            rgba(1.0, 1.0, 1.0, 0.12)
        } else {
            rgba(1.0, 1.0, 1.0, 0.55)
        },
        1.0,
    );

    let tiles: Vec<(&str, Option<&str>, Option<&str>)> = if show_three {
        vec![
            (&hh_str, ampm, None),
            (&mm_str, None, None),
            (&ss_str, None, None),
        ]
    } else {
        let sec_badge = if settings.show_seconds {
            Some(ss_str.as_str())
        } else {
            None
        };
        vec![(&hh_str, ampm, None), (&mm_str, None, sec_badge)]
    };

    let count = tiles.len();
    let inner_pad = (m * 0.032).clamp(3.0, 8.0);
    let gap = (m * 0.042).clamp(4.0, 10.0);
    let avail_w =
        (tray_r.right - tray_r.left - inner_pad * 2.0 - gap * (count as f32 - 1.0)).max(24.0);
    let tile_w = avail_w / count as f32;
    let tile_top = tray_r.top + inner_pad;
    let tile_bot = tray_r.bottom - inner_pad;
    let tile_h = (tile_bot - tile_top).max(24.0);
    let tile_rad = (m * 0.075).clamp(5.0, 14.0);
    let mid_y = (tile_top + tile_bot) * 0.5;
    let pin_w = (tile_w * 0.055).clamp(2.8, 5.8);
    let digit_sz = (tile_h * 0.52)
        .min((tile_w - pin_w * 2.0) * 0.68)
        .clamp(15.0, 118.0);

    for (ti, (digits, top_badge, bot_badge)) in tiles.into_iter().enumerate() {
        let tx = tray_r.left + inner_pad + ti as f32 * (tile_w + gap);
        let tr_tile = rect(tx, tile_top, tx + tile_w, tile_bot);

        // Frosted Liquid Glass flip flap base
        let flap_bg = if is_dark {
            rgba(0.16, 0.175, 0.21, 0.56)
        } else {
            rgba(1.0, 1.0, 1.0, 0.66)
        };
        fill_round_rect(ctx.dc, tr_tile, tile_rad, flap_bg);

        // Upper flap glossy glass highlight sheen (catches light above the split hinge!)
        let upper_r = rect(
            tr_tile.left + 0.8,
            tr_tile.top + 0.8,
            tr_tile.right - 0.8,
            mid_y,
        );
        fill_round_rect(
            ctx.dc,
            upper_r,
            (tile_rad - 1.0).max(2.0),
            if is_dark {
                rgba(1.0, 1.0, 1.0, 0.075)
            } else {
                rgba(1.0, 1.0, 1.0, 0.34)
            },
        );

        // Lower flap subtle recessed depth shading
        let lower_r = rect(
            tr_tile.left + 0.8,
            mid_y,
            tr_tile.right - 0.8,
            tr_tile.bottom - 0.8,
        );
        fill_round_rect(
            ctx.dc,
            lower_r,
            (tile_rad - 1.0).max(2.0),
            rgba(0.0, 0.0, 0.0, if is_dark { 0.14 } else { 0.045 }),
        );

        // Specular flap rim border
        draw_round_rect(
            ctx.dc,
            tr_tile,
            tile_rad,
            if is_dark {
                rgba(1.0, 1.0, 1.0, 0.22)
            } else {
                rgba(1.0, 1.0, 1.0, 0.82)
            },
            1.1,
        );

        // Tall bold split-flap numerals inset cleanly between the left & right hinge pins
        let digit_shadow = if is_dark {
            Some(rgba(0.0, 0.0, 0.0, 0.45))
        } else {
            Some(rgba(1.0, 1.0, 1.0, 0.35))
        };
        let text_box = rect(
            tr_tile.left + pin_w + 1.5,
            tr_tile.top,
            tr_tile.right - pin_w - 1.5,
            tr_tile.bottom,
        );
        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            digits,
            text_box,
            ctx.font_family,
            digit_sz,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            digit_shadow,
            false,
        );

        // Optional AM/PM top-left badge
        if let Some(ap) = top_badge {
            let b_sz = (tile_h * 0.11).clamp(7.5, 14.0);
            draw_text(
                ctx.gfx,
                ctx.dc,
                ap,
                rect(
                    tr_tile.left + 5.0,
                    tr_tile.top + 4.0,
                    tr_tile.right - 5.0,
                    tr_tile.top + b_sz + 8.0,
                ),
                ctx.font_family,
                b_sz,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.muted(0.90),
                false,
            );
        }

        // Optional compact seconds badge at bottom-right when square
        if let Some(sb) = bot_badge {
            let b_sz = (tile_h * 0.11).clamp(7.5, 14.0);
            draw_text(
                ctx.gfx,
                ctx.dc,
                sb,
                rect(
                    tr_tile.left + 5.0,
                    tr_tile.bottom - b_sz - 7.0,
                    tr_tile.right - 6.0,
                    tr_tile.bottom - 3.0,
                ),
                ctx.font_family,
                b_sz,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_TRAILING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.accent(0.95),
                false,
            );
        }

        // Crisp dark mechanical horizontal split-seam cutting across the numerals + bottom catchlight!
        let seam_h = (tile_h * 0.022).clamp(1.6, 3.4);
        fill_round_rect(
            ctx.dc,
            rect(
                tr_tile.left,
                mid_y - seam_h * 0.5,
                tr_tile.right,
                mid_y + seam_h * 0.5,
            ),
            0.0,
            rgba(0.0, 0.0, 0.0, if is_dark { 0.72 } else { 0.34 }),
        );
        fill_round_rect(
            ctx.dc,
            rect(
                tr_tile.left + 1.0,
                mid_y + seam_h * 0.5,
                tr_tile.right - 1.0,
                mid_y + seam_h * 0.5 + 0.9,
            ),
            0.0,
            rgba(1.0, 1.0, 1.0, if is_dark { 0.14 } else { 0.45 }),
        );

        // Left & right cylindrical metallic-glass hinge pins right on the split line
        let pin_h = (tile_h * 0.165).clamp(9.5, 22.0);
        for px in [tr_tile.left + 0.8, tr_tile.right - pin_w - 0.8] {
            let slot_r = rect(
                px - 0.8,
                mid_y - pin_h * 0.5 - 1.0,
                px + pin_w + 0.8,
                mid_y + pin_h * 0.5 + 1.0,
            );
            fill_round_rect(ctx.dc, slot_r, pin_w * 0.5, rgba(0.0, 0.0, 0.0, 0.58));
            let pin_r = rect(px, mid_y - pin_h * 0.5, px + pin_w, mid_y + pin_h * 0.5);
            fill_round_rect(
                ctx.dc,
                pin_r,
                pin_w * 0.45,
                if is_dark {
                    rgba(0.70, 0.74, 0.82, 0.65)
                } else {
                    rgba(0.45, 0.48, 0.55, 0.65)
                },
            );
            // Tiny glossy highlight glint on top half of hinge pin
            fill_round_rect(
                ctx.dc,
                rect(
                    px + pin_w * 0.25,
                    mid_y - pin_h * 0.34,
                    px + pin_w * 0.75,
                    mid_y - pin_h * 0.04,
                ),
                pin_w * 0.2,
                rgba(1.0, 1.0, 1.0, 0.75),
            );
        }
    }
}

pub fn render_digital(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: ClockModel = layout.parse_settings();
    let tz = find_tz(ctx.timezones, settings.time_zone_id.as_deref());
    let dt = now_in_timezone(tz);

    let time_str = clock_rs::port::format_digital_time(
        i64::from(dt.hour()),
        i64::from(dt.minute()),
        i64::from(dt.second()),
        settings.use_24_hours,
        settings.show_seconds,
    )
    .unwrap_or_else(|_| format!("{:02}:{:02}", dt.hour(), dt.minute()));

    let pad = 12.0;
    let avail_w = (ctx.width - pad * 2.0).max(20.0);
    let char_count = time_str.chars().count().max(5) as f32;
    let time_size = (avail_w / (char_count * 0.62))
        .min(ctx.height * if settings.show_date { 0.40 } else { 0.54 })
        .clamp(14.0, 96.0);

    let shadow = ctx.text_shadow();
    if settings.show_date {
        let date_str = format!(
            "{}, {} {}",
            weekday_full(ctx.lang, dt.weekday().num_days_from_sunday() as usize),
            month_full(ctx.lang, dt.month()),
            dt.day()
        );
        let date_size = (time_size * 0.34).clamp(10.0, 24.0);
        let mid_y = ctx.height * 0.54;
        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            &time_str,
            rect(pad, pad, ctx.width - pad, mid_y + time_size * 0.2),
            ctx.font_family,
            time_size,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            shadow,
            false,
        );
        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            &date_str,
            rect(
                pad,
                mid_y + time_size * 0.12,
                ctx.width - pad,
                ctx.height - pad,
            ),
            ctx.font_family,
            date_size,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.95),
            shadow,
            false,
        );
    } else {
        draw_text_shadowed(
            ctx.gfx,
            ctx.dc,
            &time_str,
            rect(pad, pad, ctx.width - pad, ctx.height - pad),
            ctx.font_family,
            time_size,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            shadow,
            false,
        );
    }
}

pub fn render_world(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: WorldClockModel = layout.parse_settings();
    let is_wide = clock_rs::port::world_is_wide_layout(f64::from(ctx.width), f64::from(ctx.height))
        .unwrap_or(false);

    let local_now = Local::now().naive_local();
    let pad = 10.0;
    let cols = if is_wide { 4 } else { 2 };
    let rows = if is_wide { 1 } else { 2 };
    let cell_w = (ctx.width - pad * 2.0) / cols as f32;
    let cell_h = (ctx.height - pad * 2.0) / rows as f32;

    for idx in 0..4 {
        let tz_id = settings.time_zone_ids.get(idx).and_then(|o| o.as_deref());
        let tz = find_tz(ctx.timezones, tz_id);
        let is_local = tz.is_none();
        let dt = now_in_timezone(tz);

        let display_name = tz
            .map(|z| z.display_name.as_str())
            .unwrap_or("(UTC+00:00) Local");

        let diff_mins = (dt - local_now).num_minutes();
        let diff_mins_rounded = ((diff_mins as f64) / 15.0).round() as i64 * 15;
        let diff_str = clock_rs::port::format_time_zone_diff(diff_mins_rounded)
            .unwrap_or_else(|_| "00:00".to_string());

        let day_delta = (dt.date() - local_now.date()).num_days();
        let rel_key = clock_rs::port::relative_day_key(day_delta)
            .unwrap_or_else(|_| "Clock_Today".to_string());
        let rel_day = tr(ctx.lang, &rel_key);

        let col = idx % cols;
        let row = idx / cols;
        let x0 = pad + col as f32 * cell_w;
        let y0 = pad + row as f32 * cell_h;

        if is_wide {
            let dial_d = (cell_w * 0.82).min(cell_h * 0.56).max(20.0);
            let dial_cx = x0 + cell_w * 0.5;
            let dial_cy = y0 + dial_d * 0.5 + 2.0;
            draw_analog_dial(
                ctx,
                dial_cx,
                dial_cy,
                dial_d,
                dt,
                false,
                AnalogStyle::StyleI,
                None,
            );

            let city = if is_local {
                tr(ctx.lang, "Clock_LocalTimeZone")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("Local")
                    .to_string()
            } else {
                clock_rs::port::city_short_name(display_name.to_string())
                    .unwrap_or_else(|_| display_name.to_string())
            };

            let text_y = dial_cy + dial_d * 0.5 + 4.0;
            let city_sz = (cell_h * 0.11).clamp(9.0, 15.0);
            let sub_sz = (cell_h * 0.095).clamp(8.0, 13.0);

            draw_text(
                ctx.gfx,
                ctx.dc,
                &city,
                rect(x0, text_y, x0 + cell_w, text_y + city_sz * 1.35),
                ctx.font_family,
                city_sz,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.fg(1.0),
                false,
            );
            draw_text(
                ctx.gfx,
                ctx.dc,
                &format!("{rel_day}, {diff_str}"),
                rect(x0, text_y + city_sz * 1.3, x0 + cell_w, y0 + cell_h),
                ctx.font_family,
                sub_sz,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.muted(0.9),
                false,
            );
        } else {
            let abbrev = clock_rs::port::city_abbreviation(display_name.to_string(), is_local)
                .unwrap_or_default();
            let dial_d = (cell_w.min(cell_h) * 0.88).max(20.0);
            draw_analog_dial(
                ctx,
                x0 + cell_w * 0.5,
                y0 + cell_h * 0.5,
                dial_d,
                dt,
                false,
                AnalogStyle::StyleI,
                if abbrev.is_empty() {
                    None
                } else {
                    Some(abbrev.as_str())
                },
            );
        }
    }
}
