pub mod calendar;
pub mod clock;
pub mod monitor;
pub mod notes;
pub mod reminders;
pub mod weather;

use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
use windows::Win32::Graphics::Direct2D::{
    D2D1_INTERPOLATION_MODE_LINEAR, ID2D1Bitmap1, ID2D1DeviceContext,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
};

use crate::core_rs;
use crate::icons;
use crate::models::{Dimensions, ResolvedTheme, WeatherConfig, WidgetLayout};
use crate::platform::graphics::{
    SharedGraphics, draw_round_rect, draw_text, fill_round_rect, rect, rgba,
};
use crate::platform::system::{MonitorCollector, WinTimeZone};
use crate::weather_client::WeatherService;

#[derive(Debug, Clone, Copy)]
pub struct WidgetSubtypeMeta {
    pub widget_type: &'static str,
    pub sub_type: &'static str,
    pub title_key: &'static str,
    pub subtitle_key: &'static str,
    pub has_settings: bool,
    pub default_cols: i64,
    pub default_rows: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetCategoryMeta {
    pub widget_type: &'static str,
    pub locale_key: &'static str,
    pub icon_svg: &'static str,
    pub subtypes: &'static [WidgetSubtypeMeta],
}

pub const WIDGET_CATEGORIES: &[WidgetCategoryMeta] = &[
    WidgetCategoryMeta {
        widget_type: "Calendar",
        locale_key: "Calendar",
        icon_svg: icons::CAT_CALENDAR,
        subtypes: &[
            WidgetSubtypeMeta {
                widget_type: "Calendar",
                sub_type: "Date",
                title_key: "Calendar_Date_Title",
                subtitle_key: "Calendar_Date_Subtitle",
                has_settings: false,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Calendar",
                sub_type: "Month",
                title_key: "Calendar_Month_Title",
                subtitle_key: "Calendar_Month_Subtitle",
                has_settings: true,
                default_cols: 4,
                default_rows: 2,
            },
        ],
    },
    WidgetCategoryMeta {
        widget_type: "Clock",
        locale_key: "Clock",
        icon_svg: icons::CAT_CLOCK,
        subtypes: &[
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "AnalogI",
                title_key: "Clock_AnalogI_Title",
                subtitle_key: "Clock_Analog_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "AnalogII",
                title_key: "Clock_AnalogII_Title",
                subtitle_key: "Clock_Analog_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "AnalogIII",
                title_key: "Clock_AnalogIII_Title",
                subtitle_key: "Clock_Analog_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "Flip",
                title_key: "Clock_Flip_Title",
                subtitle_key: "Clock_Flip_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "Digital",
                title_key: "Clock_Digital_Title",
                subtitle_key: "Clock_Analog_Subtitle",
                has_settings: true,
                default_cols: 4,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Clock",
                sub_type: "World",
                title_key: "Clock_World_Title",
                subtitle_key: "Clock_World_Subtitle",
                has_settings: true,
                default_cols: 4,
                default_rows: 2,
            },
        ],
    },
    WidgetCategoryMeta {
        widget_type: "Monitor",
        locale_key: "Monitor",
        icon_svg: icons::CAT_MONITOR,
        subtypes: &[WidgetSubtypeMeta {
            widget_type: "Monitor",
            sub_type: "SingleMetric",
            title_key: "Monitor_SingleMetric_Title",
            subtitle_key: "Monitor_SingleMetric_Subtitle",
            has_settings: true,
            default_cols: 2,
            default_rows: 2,
        }],
    },
    WidgetCategoryMeta {
        widget_type: "Notes",
        locale_key: "Notes",
        icon_svg: icons::CAT_NOTES,
        subtypes: &[WidgetSubtypeMeta {
            widget_type: "Notes",
            sub_type: "Note",
            title_key: "Notes_Title",
            subtitle_key: "Notes_Subtitle",
            has_settings: false,
            default_cols: 4,
            default_rows: 4,
        }],
    },
    WidgetCategoryMeta {
        widget_type: "Reminders",
        locale_key: "Reminders",
        icon_svg: icons::CAT_REMINDERS,
        subtypes: &[WidgetSubtypeMeta {
            widget_type: "Reminders",
            sub_type: "List",
            title_key: "Reminders_List_Title",
            subtitle_key: "Reminders_List_Subtitle",
            has_settings: true,
            default_cols: 4,
            default_rows: 4,
        }],
    },
    WidgetCategoryMeta {
        widget_type: "Weather",
        locale_key: "Weather",
        icon_svg: icons::CAT_WEATHER,
        subtypes: &[
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "Forecast",
                title_key: "Weather_Forecast_Title",
                subtitle_key: "Weather_Forecast_Subtitle",
                has_settings: true,
                default_cols: 4,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "Temperature",
                title_key: "Weather_Temperature_Title",
                subtitle_key: "Weather_Temperature_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "UVIndex",
                title_key: "Weather_UVIndex_Title",
                subtitle_key: "Weather_UVIndex_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "Pressure",
                title_key: "Weather_Pressure_Title",
                subtitle_key: "Weather_Pressure_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "AirQuality",
                title_key: "Weather_AirQuality_Title",
                subtitle_key: "Weather_AirQuality_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
            WidgetSubtypeMeta {
                widget_type: "Weather",
                sub_type: "SunriseSunset",
                title_key: "Weather_SunriseSunset_Title",
                subtitle_key: "Weather_SunriseSunset_Subtitle",
                has_settings: true,
                default_cols: 2,
                default_rows: 2,
            },
        ],
    },
];

pub fn subtype_has_settings(widget_type: &str, sub_type: &str) -> bool {
    for cat in WIDGET_CATEGORIES {
        if cat.widget_type == widget_type {
            for st in cat.subtypes {
                if st.sub_type == sub_type {
                    return st.has_settings;
                }
            }
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditFieldId {
    NoteTitle,
    NoteContent,
    ReminderListName,
    ReminderItem(usize),
    ReminderNew,
}

#[derive(Debug, Clone)]
pub struct InlineEditState {
    pub field: EditFieldId,
    pub buffer: String,
    pub multiline: bool,
}

#[derive(Debug, Clone)]
pub enum WidgetHitAction {
    ToggleReminder(usize),
    BeginEdit(EditFieldId),
}

#[derive(Debug, Clone)]
pub struct WidgetHitRegion {
    pub rect: D2D_RECT_F,
    pub action: WidgetHitAction,
}

pub struct WidgetRenderCtx<'a> {
    pub gfx: &'a SharedGraphics,
    pub dc: &'a ID2D1DeviceContext,
    pub width: f32,
    pub height: f32,
    pub theme: ResolvedTheme,
    pub font_family: &'a str,
    pub lang: &'a str,
    pub monitor: &'a MonitorCollector,
    pub weather: &'a WeatherService,
    pub weather_cfg: &'a WeatherConfig,
    pub timezones: &'a [WinTimeZone],
    pub inline_edit: Option<&'a InlineEditState>,
    pub hits: &'a mut Vec<WidgetHitRegion>,
    pub is_preview: bool,
}

impl<'a> WidgetRenderCtx<'a> {
    pub fn fg(&self, alpha_mul: f32) -> D2D1_COLOR_F {
        let (r, g, b) = self.theme.fg_rgb;
        rgba(
            r,
            g,
            b,
            (self.theme.text_opacity * alpha_mul).clamp(0.0, 1.0),
        )
    }

    pub fn muted(&self, alpha_mul: f32) -> D2D1_COLOR_F {
        let (r, g, b) = self.theme.fg_muted;
        rgba(
            r,
            g,
            b,
            (self.theme.text_opacity * alpha_mul).clamp(0.0, 1.0),
        )
    }

    pub fn accent(&self, alpha_mul: f32) -> D2D1_COLOR_F {
        let (r, g, b) = self.theme.accent_effective;
        rgba(
            r,
            g,
            b,
            (self.theme.text_opacity * alpha_mul).clamp(0.0, 1.0),
        )
    }

    /// Subtle optical contrast shadow for text over translucent glass surfaces.
    pub fn text_shadow(&self) -> Option<D2D1_COLOR_F> {
        if !self.theme.uses_acrylic {
            return None;
        }
        if self.theme.is_dark {
            Some(rgba(0.0, 0.0, 0.0, 0.42))
        } else {
            Some(rgba(1.0, 1.0, 1.0, 0.35))
        }
    }

    pub fn push_hit(&mut self, r: D2D_RECT_F, action: WidgetHitAction) {
        if !self.is_preview {
            self.hits.push(WidgetHitRegion { rect: r, action });
        }
    }
}

pub fn scale_matrix(sx: f32, sy: f32) -> Matrix3x2 {
    Matrix3x2 {
        M11: sx,
        M12: 0.0,
        M21: 0.0,
        M22: sy,
        M31: 0.0,
        M32: 0.0,
    }
}

pub fn translate_matrix(dx: f32, dy: f32) -> Matrix3x2 {
    Matrix3x2 {
        M11: 1.0,
        M12: 0.0,
        M21: 0.0,
        M22: 1.0,
        M31: dx,
        M32: dy,
    }
}

pub fn rotate_around(angle_deg: f32, cx: f32, cy: f32) -> Matrix3x2 {
    let rad = angle_deg.to_radians();
    let (s, c) = rad.sin_cos();
    Matrix3x2 {
        M11: c,
        M12: s,
        M21: -s,
        M22: c,
        M31: cx - c * cx + s * cy,
        M32: cy - s * cx - c * cy,
    }
}

/// Renders a widget card (Liquid Glass optical bitmap or solid/acrylic card, border, scaled content) into `bounds`.
#[allow(clippy::too_many_arguments)]
pub fn render_widget_card(
    gfx: &SharedGraphics,
    dc: &ID2D1DeviceContext,
    layout: &WidgetLayout,
    bounds: D2D_RECT_F,
    theme: ResolvedTheme,
    dims: &Dimensions,
    font_family: &str,
    lang: &str,
    monitor: &MonitorCollector,
    weather: &WeatherService,
    weather_cfg: &WeatherConfig,
    timezones: &[WinTimeZone],
    inline_edit: Option<&InlineEditState>,
    hits: &mut Vec<WidgetHitRegion>,
    is_preview: bool,
    glass_bg: Option<&ID2D1Bitmap1>,
) {
    let w = (bounds.right - bounds.left).max(1.0);
    let h = (bounds.bottom - bounds.top).max(1.0);

    // Draw card surface: real SDF Liquid Glass bitmap when available, else frosted/acrylic/solid card
    let (br, bg, bb) = theme.bg_rgb;
    let radius = if is_preview {
        if theme.use_native_frame {
            6.0
        } else {
            (dims.radius as f32 * 0.65).clamp(6.0, 20.0)
        }
    } else if theme.use_native_frame {
        8.0
    } else {
        theme.radius
    };

    if let Some(glass_bmp) = glass_bg {
        unsafe {
            dc.DrawBitmap(
                glass_bmp,
                Some(&bounds),
                1.0,
                D2D1_INTERPOLATION_MODE_LINEAR,
                None,
                None,
            );
        }
    } else {
        fill_round_rect(dc, bounds, radius, rgba(br, bg, bb, theme.bg_alpha));

        // Glass sheen + specular Fresnel rim preview when Liquid Glass / Acrylic is active
        if theme.uses_acrylic && !theme.use_native_frame {
            let sheen_h = (h * 0.42).min(64.0);
            fill_round_rect(
                dc,
                rect(
                    bounds.left + 1.0,
                    bounds.top + 1.0,
                    bounds.right - 1.0,
                    bounds.top + sheen_h,
                ),
                (radius - 1.0).max(2.0),
                rgba(1.0, 1.0, 1.0, if theme.is_dark { 0.08 } else { 0.22 }),
            );
            let inset = rect(
                bounds.left + 0.75,
                bounds.top + 0.75,
                bounds.right - 0.75,
                bounds.bottom - 0.75,
            );
            draw_round_rect(
                dc,
                inset,
                (radius - 0.75).max(0.0),
                rgba(1.0, 1.0, 1.0, if theme.is_dark { 0.36 } else { 0.60 }),
                1.4,
            );
        } else if theme.use_native_frame {
            let border_col = if theme.is_dark {
                rgba(1.0, 1.0, 1.0, 0.18)
            } else {
                rgba(0.0, 0.0, 0.0, 0.14)
            };
            let inset = rect(
                bounds.left + 0.5,
                bounds.top + 0.5,
                bounds.right - 0.5,
                bounds.bottom - 0.5,
            );
            draw_round_rect(dc, inset, (radius - 0.5).max(0.0), border_col, 1.0);
        }
    }

    // Apply LayoutTransformControl scaling (`Dimensions.Size / 72.0`), or 1.0 for fixed preview cards
    let scale = if is_preview {
        1.0_f32
    } else {
        core_rs::port::content_scale(i64::from(dims.size))
            .unwrap_or(1.0)
            .clamp(0.4, 3.0) as f32
    };
    let logical_w = w / scale;
    let logical_h = h / scale;

    let mut prev_transform = Matrix3x2::identity();
    unsafe {
        dc.GetTransform(&mut prev_transform);
        dc.PushAxisAlignedClip(
            &bounds,
            windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        );
        let xf =
            scale_matrix(scale, scale) * translate_matrix(bounds.left, bounds.top) * prev_transform;
        dc.SetTransform(&xf);
    }

    let mut local_hits = Vec::new();
    {
        let mut ctx = WidgetRenderCtx {
            gfx,
            dc,
            width: logical_w,
            height: logical_h,
            theme,
            font_family,
            lang,
            monitor,
            weather,
            weather_cfg,
            timezones,
            inline_edit,
            hits: &mut local_hits,
            is_preview,
        };
        render_widget_content(&mut ctx, layout);
    }

    // Transform hit regions from logical widget coordinates back to window DIP coordinates
    for mut hr in local_hits {
        hr.rect = rect(
            bounds.left + hr.rect.left * scale,
            bounds.top + hr.rect.top * scale,
            bounds.left + hr.rect.right * scale,
            bounds.top + hr.rect.bottom * scale,
        );
        hits.push(hr);
    }

    unsafe {
        dc.SetTransform(&prev_transform);
        dc.PopAxisAlignedClip();
    }
}

pub fn render_widget_content(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    match (layout.widget_type.as_str(), layout.sub_type.as_str()) {
        ("Calendar", "Date") => calendar::render_date(ctx),
        ("Calendar", "Month") => calendar::render_month(ctx, layout),
        ("Clock", "AnalogI") => clock::render_analog(ctx, layout, clock::AnalogStyle::StyleI),
        ("Clock", "AnalogII") => clock::render_analog(ctx, layout, clock::AnalogStyle::StyleII),
        ("Clock", "AnalogIII") => clock::render_analog(ctx, layout, clock::AnalogStyle::StyleIII),
        ("Clock", "Flip") => clock::render_flip(ctx, layout),
        ("Clock", "Digital") => clock::render_digital(ctx, layout),
        ("Clock", "World") => clock::render_world(ctx, layout),
        ("Monitor", "SingleMetric") => monitor::render_single_metric(ctx, layout),
        ("Notes", "Note") => notes::render_note(ctx, layout),
        ("Reminders", "List") => reminders::render_list(ctx, layout),
        ("Weather", "Forecast") => weather::render_forecast(ctx, layout),
        ("Weather", "Temperature") => weather::render_temperature(ctx, layout),
        ("Weather", "UVIndex") => weather::render_uv_index(ctx, layout),
        ("Weather", "Pressure") => weather::render_pressure(ctx, layout),
        ("Weather", "AirQuality") => weather::render_air_quality(ctx, layout),
        ("Weather", "SunriseSunset") => weather::render_sunrise_sunset(ctx, layout),
        _ => {
            draw_text(
                ctx.gfx,
                ctx.dc,
                &format!("{}/{}", layout.widget_type, layout.sub_type),
                rect(8.0, 8.0, ctx.width - 8.0, ctx.height - 8.0),
                ctx.font_family,
                13.0,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                ctx.muted(1.0),
                true,
            );
        }
    }
}
