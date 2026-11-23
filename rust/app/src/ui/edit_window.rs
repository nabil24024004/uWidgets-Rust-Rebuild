use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics,
    IDC_ARROW, LoadCursorW, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SetForegroundWindow, ShowWindow, WM_CHAR, WM_CLOSE, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEWHEEL,
    WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_CAPTION, WS_EX_NOREDIRECTIONBITMAP, WS_SYSMENU, WS_VISIBLE,
};
use windows_core::PCWSTR;

use crate::i18n::{tr, weekday_full};
use crate::models::{
    ClockModel, ForecastModel, MonthCalendarModel, RemindersListModel, SingleMetricModel,
    WorldClockModel,
};
use crate::monitor_rs;
use crate::notes_reminders_rs;
use crate::platform::graphics::{
    WindowSurface, draw_round_rect, draw_text, fill_circle, fill_round_rect, rect, rgba,
};
use crate::platform::system::{create_app_icon, lparam_point, to_wide};
use crate::ui::{AppCommand, AppState, enqueue_cmd, get_global_app, settings_window};
use crate::weather_client::GeoCity;

const CLASS_NAME: &str = "uWidgetsRsEditWidget";

#[derive(Debug, Clone)]
enum EditAction {
    Done,
    SetFirstDay(i32),
    ToggleClockSeconds,
    ToggleClockDate,
    ToggleClock24H,
    SelectTzSlot(usize),
    PickTimezone(Option<String>),
    SetMonitorMetric(i32),
    DeleteCompletedReminders,
    DeleteAllReminders,
    SetTempUnit(String),
    TriggerCitySearch,
    FocusCityInput,
    PickCity(usize),
}

pub struct EditWindow {
    pub hwnd: HWND,
    pub widget_id: u64,
    pub surface: WindowSurface,
    pub active_tz_slot: usize,
    pub tz_scroll: usize,
    pub city_query: String,
    pub city_input_focused: bool,
    pub city_results: Vec<GeoCity>,
    pub city_status: Option<String>,
    hits: Vec<(D2D_RECT_F, EditAction)>,
}

impl EditWindow {
    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, false);
        }
    }
}

pub fn register_class() {
    let class_w = to_wide(CLASS_NAME);
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hIcon: create_app_icon(32).unwrap_or_default(),
            hIconSm: create_app_icon(16).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
    }
}

pub fn open_for_widget(app: &Rc<RefCell<AppState>>, widget_id: u64) {
    // Close any previous EditWindow first (drop borrow BEFORE DestroyWindow!)
    let old_hwnd = { app.borrow_mut().edit_win.take().map(|e| e.hwnd) };
    if let Some(old_hwnd) = old_hwnd {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(old_hwnd);
        }
    }

    let (title, is_dark, gfx, init_city) = {
        let st = app.borrow();
        let Some(layout) = st.find_layout(widget_id) else {
            return;
        };
        let city = if layout.widget_type == "Weather" {
            layout.parse_settings::<ForecastModel>().name
        } else {
            String::new()
        };
        (
            format!(
                "{} — {} / {}",
                tr(&st.settings.region.language, "Widget_Edit"),
                layout.widget_type,
                layout.sub_type
            ),
            st.resolved_theme().is_dark,
            Rc::clone(&st.gfx),
            city,
        )
    };

    let dpi_scale = unsafe {
        let hdc = windows::Win32::Graphics::Gdi::GetDC(None);
        let logpx = windows::Win32::Graphics::Gdi::GetDeviceCaps(
            hdc,
            windows::Win32::Graphics::Gdi::LOGPIXELSX,
        );
        windows::Win32::Graphics::Gdi::ReleaseDC(None, hdc);
        (logpx.max(96) as f32) / 96.0
    };
    let w = (440.0 * dpi_scale).round() as i32;
    let h = (480.0 * dpi_scale).round() as i32;
    let (x, y) = unsafe {
        let sx = GetSystemMetrics(SM_CXSCREEN);
        let sy = GetSystemMetrics(SM_CYSCREEN);
        ((sx - w) / 2, (sy - h) / 2)
    };
    let class_w = to_wide(CLASS_NAME);
    let title_w = to_wide(&title);

    let hwnd = unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x,
            y,
            w,
            h,
            None,
            None,
            hinst,
            None,
        )
    };
    let Ok(hwnd) = hwnd else { return };
    settings_window::apply_titlebar_theme(hwnd, is_dark);

    let mut rc = RECT::default();
    let dpi = unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
        (GetDpiForWindow(hwnd) as f32).max(96.0)
    };
    let cw = (rc.right - rc.left).max(1) as u32;
    let ch = (rc.bottom - rc.top).max(1) as u32;

    if let Ok(surface) = WindowSurface::new(gfx, hwnd, cw, ch, dpi) {
        app.borrow_mut().edit_win = Some(EditWindow {
            hwnd,
            widget_id,
            surface,
            active_tz_slot: 0,
            tz_scroll: 0,
            city_query: init_city,
            city_input_focused: false,
            city_results: Vec::new(),
            city_status: None,
            hits: Vec::new(),
        });
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = InvalidateRect(hwnd, None, false);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_btn(
    ew: &EditWindow,
    is_dark: bool,
    accent: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    fg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    r: D2D_RECT_F,
    label: &str,
    font: &str,
    active: bool,
) {
    let bg = if active {
        accent
    } else if is_dark {
        rgba(1.0, 1.0, 1.0, 0.09)
    } else {
        rgba(0.0, 0.0, 0.0, 0.06)
    };
    fill_round_rect(&ew.surface.dc, r, 7.0, bg);
    if !active {
        draw_round_rect(
            &ew.surface.dc,
            r,
            7.0,
            if is_dark {
                rgba(1.0, 1.0, 1.0, 0.12)
            } else {
                rgba(0.0, 0.0, 0.0, 0.12)
            },
            1.0,
        );
    }
    draw_text(
        &ew.surface.gfx,
        &ew.surface.dc,
        label,
        r,
        font,
        12.5,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        if active { rgba(1.0, 1.0, 1.0, 1.0) } else { fg },
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_toggle_row(
    ew: &EditWindow,
    is_dark: bool,
    accent: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    fg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    y: f32,
    w: f32,
    label: &str,
    font: &str,
    on: bool,
) -> D2D_RECT_F {
    let row = rect(18.0, y, w - 18.0, y + 40.0);
    fill_round_rect(
        &ew.surface.dc,
        row,
        8.0,
        if is_dark {
            rgba(1.0, 1.0, 1.0, 0.05)
        } else {
            rgba(0.0, 0.0, 0.0, 0.04)
        },
    );
    draw_text(
        &ew.surface.gfx,
        &ew.surface.dc,
        label,
        rect(row.left + 12.0, row.top, row.right - 64.0, row.bottom),
        font,
        13.0,
        DWRITE_FONT_WEIGHT_MEDIUM,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        fg,
        false,
    );
    let cy = (row.top + row.bottom) * 0.5;
    let tog = rect(row.right - 54.0, cy - 11.0, row.right - 12.0, cy + 11.0);
    fill_round_rect(
        &ew.surface.dc,
        tog,
        11.0,
        if on {
            accent
        } else if is_dark {
            rgba(1.0, 1.0, 1.0, 0.22)
        } else {
            rgba(0.0, 0.0, 0.0, 0.22)
        },
    );
    fill_circle(
        &ew.surface.dc,
        if on {
            tog.right - 11.0
        } else {
            tog.left + 11.0
        },
        cy,
        8.0,
        rgba(1.0, 1.0, 1.0, 0.96),
    );
    row
}

fn paint_edit(st: &mut AppState) {
    let theme = st.resolved_theme();
    let lang = st.settings.region.language.clone();
    let font = st.settings.theme.font_family.clone();
    let widget_id = match st.edit_win.as_ref() {
        Some(e) => e.widget_id,
        None => return,
    };
    let Some(layout) = st.find_layout(widget_id).cloned() else {
        return;
    };
    let timezones = st.timezones.clone();

    let Some(ew) = st.edit_win.as_mut() else {
        return;
    };
    let w = ew.surface.width as f32 * 96.0 / ew.surface.dpi;
    let h = ew.surface.height as f32 * 96.0 / ew.surface.dpi;

    let bg = if theme.is_dark {
        rgba(0.12, 0.125, 0.14, 1.0)
    } else {
        rgba(0.96, 0.965, 0.975, 1.0)
    };
    let fg = if theme.is_dark {
        rgba(0.97, 0.97, 0.98, 1.0)
    } else {
        rgba(0.10, 0.10, 0.12, 1.0)
    };
    let muted = if theme.is_dark {
        rgba(0.68, 0.70, 0.74, 1.0)
    } else {
        rgba(0.42, 0.44, 0.48, 1.0)
    };
    let (ar, ag, ab) = theme.accent_rgb;
    let accent = rgba(ar, ag, ab, 1.0);

    let mut hits = Vec::new();
    ew.surface.begin();
    fill_round_rect(&ew.surface.dc, rect(0.0, 0.0, w, h), 0.0, bg);

    let mut y = 16.0_f32;
    draw_text(
        &ew.surface.gfx,
        &ew.surface.dc,
        &format!("{} — {}", layout.widget_type, layout.sub_type),
        rect(18.0, y, w - 18.0, y + 28.0),
        &font,
        16.5,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        fg,
        false,
    );
    y += 38.0;

    match (layout.widget_type.as_str(), layout.sub_type.as_str()) {
        ("Calendar", "Month") => {
            let model: MonthCalendarModel = layout.parse_settings();
            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                tr(&lang, "Calendar_FirstDayOfWeek"),
                rect(18.0, y, w - 18.0, y + 24.0),
                &font,
                13.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                muted,
                false,
            );
            y += 30.0;
            for d in 0..7 {
                let r = rect(18.0, y, w - 18.0, y + 36.0);
                draw_btn(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    r,
                    weekday_full(&lang, d as usize),
                    &font,
                    model.first_day_of_week == d,
                );
                hits.push((r, EditAction::SetFirstDay(d)));
                y += 42.0;
            }
        }
        ("Clock", "AnalogI" | "AnalogII" | "AnalogIII" | "Flip" | "Digital") => {
            let model: ClockModel = layout.parse_settings();
            let r_sec = draw_toggle_row(
                ew,
                theme.is_dark,
                accent,
                fg,
                y,
                w,
                tr(&lang, "Clock_ShowSeconds"),
                &font,
                model.show_seconds,
            );
            hits.push((r_sec, EditAction::ToggleClockSeconds));
            y += 46.0;

            if layout.sub_type == "Digital" {
                let r_date = draw_toggle_row(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    y,
                    w,
                    tr(&lang, "Clock_ShowDate"),
                    &font,
                    model.show_date,
                );
                hits.push((r_date, EditAction::ToggleClockDate));
                y += 46.0;
            }

            if matches!(layout.sub_type.as_str(), "Digital" | "Flip") {
                let r_24 = draw_toggle_row(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    y,
                    w,
                    tr(&lang, "Clock_Use24Hours"),
                    &font,
                    model.use_24_hours,
                );
                hits.push((r_24, EditAction::ToggleClock24H));
                y += 46.0;
            }

            draw_tz_picker(
                ew,
                theme.is_dark,
                accent,
                fg,
                muted,
                &lang,
                &font,
                &timezones,
                model.time_zone_id.as_deref(),
                y,
                w,
                h - 66.0,
                &mut hits,
            );
        }
        ("Clock", "World") => {
            let model: WorldClockModel = layout.parse_settings();
            // 4 slot tabs
            let tab_w = (w - 36.0 - 18.0) / 4.0;
            for slot in 0..4 {
                let tx = 18.0 + slot as f32 * (tab_w + 6.0);
                let tr_r = rect(tx, y, tx + tab_w, y + 32.0);
                draw_btn(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    tr_r,
                    &format!("#{}", slot + 1),
                    &font,
                    ew.active_tz_slot == slot,
                );
                hits.push((tr_r, EditAction::SelectTzSlot(slot)));
            }
            y += 42.0;

            let cur_tz = model
                .time_zone_ids
                .get(ew.active_tz_slot)
                .and_then(|o| o.as_deref());
            draw_tz_picker(
                ew,
                theme.is_dark,
                accent,
                fg,
                muted,
                &lang,
                &font,
                &timezones,
                cur_tz,
                y,
                w,
                h - 66.0,
                &mut hits,
            );
        }
        ("Monitor", "SingleMetric") => {
            let model: SingleMetricModel = layout.parse_settings();
            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                tr(&lang, "Monitor_Metric"),
                rect(18.0, y, w - 18.0, y + 24.0),
                &font,
                13.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                muted,
                false,
            );
            y += 30.0;
            for m in 0..5 {
                let key = monitor_rs::port::metric_locale_key(i64::from(m))
                    .unwrap_or_else(|_| "Monitor_Metric_0".to_string());
                let r = rect(18.0, y, w - 18.0, y + 40.0);
                draw_btn(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    r,
                    tr(&lang, &key),
                    &font,
                    model.metric == m,
                );
                hits.push((r, EditAction::SetMonitorMetric(m)));
                y += 48.0;
            }
        }
        ("Reminders", "List") => {
            let model: RemindersListModel = layout.parse_settings();
            let flags: Vec<bool> = model.reminders.iter().map(|r| r.completed).collect();
            let rem = notes_reminders_rs::port::count_remaining(flags.clone()).unwrap_or(0);
            let tot = notes_reminders_rs::port::count_total(flags).unwrap_or(0);
            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                &format!(
                    "{}: {rem} / {tot}",
                    model.list_name.as_deref().unwrap_or(tr(&lang, "Reminders"))
                ),
                rect(18.0, y, w - 18.0, y + 28.0),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                muted,
                false,
            );
            y += 40.0;
            let r_dc = rect(18.0, y, w - 18.0, y + 40.0);
            draw_btn(
                ew,
                theme.is_dark,
                accent,
                fg,
                r_dc,
                tr(&lang, "Reminders_DeleteCompleted"),
                &font,
                false,
            );
            hits.push((r_dc, EditAction::DeleteCompletedReminders));
            y += 50.0;
            let r_da = rect(18.0, y, w - 18.0, y + 40.0);
            draw_btn(
                ew,
                theme.is_dark,
                rgba(0.85, 0.22, 0.22, 1.0),
                fg,
                r_da,
                tr(&lang, "Reminders_DeleteAll"),
                &font,
                true,
            );
            hits.push((r_da, EditAction::DeleteAllReminders));
        }
        ("Weather", _) => {
            let model: ForecastModel = layout.parse_settings();
            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                tr(&lang, "Weather_Units"),
                rect(18.0, y, w - 18.0, y + 22.0),
                &font,
                12.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                muted,
                false,
            );
            y += 26.0;
            let half = (w - 44.0) * 0.5;
            let r_c = rect(18.0, y, 18.0 + half, y + 36.0);
            let r_f = rect(26.0 + half, y, w - 18.0, y + 36.0);
            draw_btn(
                ew,
                theme.is_dark,
                accent,
                fg,
                r_c,
                tr(&lang, "Weather_Units_Celsius"),
                &font,
                model.temperature_unit.eq_ignore_ascii_case("celsius"),
            );
            draw_btn(
                ew,
                theme.is_dark,
                accent,
                fg,
                r_f,
                tr(&lang, "Weather_Units_Fahrenheit"),
                &font,
                model.temperature_unit.eq_ignore_ascii_case("fahrenheit"),
            );
            hits.push((r_c, EditAction::SetTempUnit("celsius".to_string())));
            hits.push((r_f, EditAction::SetTempUnit("fahrenheit".to_string())));
            y += 48.0;

            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                &format!(
                    "{}: {} ({:.2}, {:.2})",
                    tr(&lang, "Weather_Location"),
                    model.name,
                    model.latitude,
                    model.longitude
                ),
                rect(18.0, y, w - 18.0, y + 22.0),
                &font,
                12.0,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                muted,
                false,
            );
            y += 26.0;

            // Search input + Search button
            let inp_r = rect(18.0, y, w - 104.0, y + 36.0);
            fill_round_rect(
                &ew.surface.dc,
                inp_r,
                7.0,
                if theme.is_dark {
                    rgba(1.0, 1.0, 1.0, 0.08)
                } else {
                    rgba(1.0, 1.0, 1.0, 0.95)
                },
            );
            draw_round_rect(
                &ew.surface.dc,
                inp_r,
                7.0,
                if ew.city_input_focused { accent } else { muted },
                if ew.city_input_focused { 1.6 } else { 1.0 },
            );
            let disp = if ew.city_input_focused {
                format!("{}|", ew.city_query)
            } else if ew.city_query.is_empty() {
                "Type city name…".to_string()
            } else {
                ew.city_query.clone()
            };
            draw_text(
                &ew.surface.gfx,
                &ew.surface.dc,
                &disp,
                rect(
                    inp_r.left + 10.0,
                    inp_r.top,
                    inp_r.right - 8.0,
                    inp_r.bottom,
                ),
                &font,
                13.0,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                fg,
                false,
            );
            hits.push((inp_r, EditAction::FocusCityInput));

            let search_r = rect(w - 96.0, y, w - 18.0, y + 36.0);
            draw_btn(
                ew,
                theme.is_dark,
                accent,
                fg,
                search_r,
                "Search",
                &font,
                true,
            );
            hits.push((search_r, EditAction::TriggerCitySearch));
            y += 46.0;

            if let Some(status) = &ew.city_status {
                draw_text(
                    &ew.surface.gfx,
                    &ew.surface.dc,
                    status,
                    rect(18.0, y, w - 18.0, y + 22.0),
                    &font,
                    11.5,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    muted,
                    false,
                );
                y += 24.0;
            }

            let max_y = h - 66.0;
            for (ci, city) in ew.city_results.iter().enumerate() {
                if y + 34.0 > max_y {
                    break;
                }
                let r = rect(18.0, y, w - 18.0, y + 34.0);
                draw_btn(
                    ew,
                    theme.is_dark,
                    accent,
                    fg,
                    r,
                    &format!(
                        "{} ({:.2}, {:.2})",
                        city.search_name(),
                        city.latitude,
                        city.longitude
                    ),
                    &font,
                    false,
                );
                hits.push((r, EditAction::PickCity(ci)));
                y += 38.0;
            }
        }
        _ => {}
    }

    // Bottom Done button
    let done_r = rect(w - 118.0, h - 50.0, w - 18.0, h - 16.0);
    draw_btn(
        ew,
        theme.is_dark,
        accent,
        fg,
        done_r,
        tr(&lang, "Widget_Save"),
        &font,
        true,
    );
    hits.push((done_r, EditAction::Done));

    ew.hits = hits;
    let _ = ew.surface.end();
}

#[allow(clippy::too_many_arguments)]
fn draw_tz_picker(
    ew: &EditWindow,
    is_dark: bool,
    accent: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    fg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    muted: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    lang: &str,
    font: &str,
    zones: &[crate::platform::system::WinTimeZone],
    selected_id: Option<&str>,
    mut y: f32,
    w: f32,
    max_y: f32,
    hits: &mut Vec<(D2D_RECT_F, EditAction)>,
) {
    draw_text(
        &ew.surface.gfx,
        &ew.surface.dc,
        &format!("{} (scroll wheel)", tr(lang, "Clock_TimeZone")),
        rect(18.0, y, w - 18.0, y + 22.0),
        font,
        12.0,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        muted,
        false,
    );
    y += 26.0;

    // Option 0: Local Time Zone
    let local_r = rect(18.0, y, w - 18.0, y + 32.0);
    draw_btn(
        ew,
        is_dark,
        accent,
        fg,
        local_r,
        tr(lang, "Clock_LocalTimeZone"),
        font,
        selected_id.is_none(),
    );
    hits.push((local_r, EditAction::PickTimezone(None)));
    y += 38.0;

    let row_h = 30.0_f32;
    let rows_avail = ((max_y - y) / (row_h + 4.0)).floor().max(1.0) as usize;
    let start = ew.tz_scroll.min(zones.len().saturating_sub(1));
    for tz in zones.iter().skip(start).take(rows_avail) {
        let r = rect(18.0, y, w - 18.0, y + row_h);
        let sel = selected_id.is_some_and(|s| s.eq_ignore_ascii_case(&tz.id));
        fill_round_rect(
            &ew.surface.dc,
            r,
            6.0,
            if sel {
                accent
            } else if is_dark {
                rgba(1.0, 1.0, 1.0, 0.06)
            } else {
                rgba(0.0, 0.0, 0.0, 0.05)
            },
        );
        draw_text(
            &ew.surface.gfx,
            &ew.surface.dc,
            &tz.display_name,
            rect(r.left + 10.0, r.top, r.right - 8.0, r.bottom),
            font,
            11.5,
            if sel {
                DWRITE_FONT_WEIGHT_SEMI_BOLD
            } else {
                DWRITE_FONT_WEIGHT_NORMAL
            },
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            if sel { rgba(1.0, 1.0, 1.0, 1.0) } else { fg },
            false,
        );
        hits.push((r, EditAction::PickTimezone(Some(tz.id.clone()))));
        y += row_h + 4.0;
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                paint_edit(&mut st);
            }
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                if let Some(ew) = st.edit_win.as_mut() {
                    let mut rc = RECT::default();
                    let dpi = unsafe {
                        let _ = GetClientRect(hwnd, &mut rc);
                        (GetDpiForWindow(hwnd) as f32).max(96.0)
                    };
                    let _ = ew.surface.resize(
                        (rc.right - rc.left).max(1) as u32,
                        (rc.bottom - rc.top).max(1) as u32,
                        dpi,
                    );
                }
            }
            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as i16;
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                let max_tz = st.timezones.len().saturating_sub(5);
                if let Some(ew) = st.edit_win.as_mut() {
                    if delta < 0 {
                        ew.tz_scroll = (ew.tz_scroll + 3).min(max_tz);
                    } else {
                        ew.tz_scroll = ew.tz_scroll.saturating_sub(3);
                    }
                    ew.invalidate();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (px, py) = lparam_point(lparam);
            if let Some(app) = get_global_app() {
                let hit = {
                    let st = app.borrow();
                    st.edit_win.as_ref().and_then(|ew| {
                        let dx = px as f32 * 96.0 / ew.surface.dpi;
                        let dy = py as f32 * 96.0 / ew.surface.dpi;
                        ew.hits
                            .iter()
                            .find(|(r, _)| {
                                dx >= r.left && dx <= r.right && dy >= r.top && dy <= r.bottom
                            })
                            .map(|(_, a)| (ew.widget_id, a.clone()))
                    })
                };
                if let Some((wid, action)) = hit {
                    let mut st = app.borrow_mut();
                    let mut save_and_repaint = false;
                    let mut weather_changed = false;
                    match action {
                        EditAction::Done => {
                            drop(st);
                            enqueue_cmd(AppCommand::CloseEditWidget);
                            return LRESULT(0);
                        }
                        EditAction::SetFirstDay(d) => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                l.set_settings(&MonthCalendarModel {
                                    first_day_of_week: d,
                                });
                                save_and_repaint = true;
                            }
                        }
                        EditAction::ToggleClockSeconds => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: ClockModel = l.parse_settings();
                                m.show_seconds = !m.show_seconds;
                                l.set_settings(&m);
                                save_and_repaint = true;
                            }
                        }
                        EditAction::ToggleClockDate => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: ClockModel = l.parse_settings();
                                m.show_date = !m.show_date;
                                l.set_settings(&m);
                                save_and_repaint = true;
                            }
                        }
                        EditAction::ToggleClock24H => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: ClockModel = l.parse_settings();
                                m.use_24_hours = !m.use_24_hours;
                                l.set_settings(&m);
                                save_and_repaint = true;
                            }
                        }
                        EditAction::SelectTzSlot(slot) => {
                            if let Some(ew) = st.edit_win.as_mut() {
                                ew.active_tz_slot = slot;
                                ew.invalidate();
                            }
                        }
                        EditAction::PickTimezone(tz_id) => {
                            let slot = st.edit_win.as_ref().map_or(0, |e| e.active_tz_slot);
                            if let Some(l) = st.find_layout_mut(wid) {
                                if l.sub_type == "World" {
                                    let mut m: WorldClockModel = l.parse_settings();
                                    while m.time_zone_ids.len() < 4 {
                                        m.time_zone_ids.push(None);
                                    }
                                    m.time_zone_ids[slot.min(3)] = tz_id;
                                    l.set_settings(&m);
                                } else {
                                    let mut m: ClockModel = l.parse_settings();
                                    m.time_zone_id = tz_id;
                                    l.set_settings(&m);
                                }
                                save_and_repaint = true;
                            }
                        }
                        EditAction::SetMonitorMetric(m) => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                l.set_settings(&SingleMetricModel { metric: m });
                                save_and_repaint = true;
                            }
                        }
                        EditAction::DeleteCompletedReminders => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: RemindersListModel = l.parse_settings();
                                m.reminders.retain(|r| !r.completed);
                                l.set_settings(&m);
                                save_and_repaint = true;
                            }
                        }
                        EditAction::DeleteAllReminders => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: RemindersListModel = l.parse_settings();
                                m.reminders.clear();
                                l.set_settings(&m);
                                save_and_repaint = true;
                            }
                        }
                        EditAction::SetTempUnit(unit) => {
                            if let Some(l) = st.find_layout_mut(wid) {
                                let mut m: ForecastModel = l.parse_settings();
                                m.temperature_unit = unit;
                                l.set_settings(&m);
                                save_and_repaint = true;
                                weather_changed = true;
                            }
                        }
                        EditAction::FocusCityInput => {
                            if let Some(ew) = st.edit_win.as_mut() {
                                ew.city_input_focused = true;
                                ew.invalidate();
                            }
                        }
                        EditAction::TriggerCitySearch => {
                            let q = st
                                .edit_win
                                .as_mut()
                                .map(|ew| {
                                    ew.city_status = Some("Searching…".to_string());
                                    ew.invalidate();
                                    ew.city_query.clone()
                                })
                                .unwrap_or_default();
                            drop(st);
                            enqueue_cmd(AppCommand::SearchWeatherCities(q));
                            return LRESULT(0);
                        }
                        EditAction::PickCity(idx) => {
                            let city = st
                                .edit_win
                                .as_ref()
                                .and_then(|ew| ew.city_results.get(idx).cloned());
                            if let Some(city) = city {
                                if let Some(l) = st.find_layout_mut(wid) {
                                    let mut m: ForecastModel = l.parse_settings();
                                    m.name = city.name.clone();
                                    m.latitude = city.latitude;
                                    m.longitude = city.longitude;
                                    l.set_settings(&m);
                                    save_and_repaint = true;
                                    weather_changed = true;
                                }
                                if let Some(ew) = st.edit_win.as_mut() {
                                    ew.city_query = city.name;
                                    ew.city_status = Some("Location updated!".to_string());
                                }
                            }
                        }
                    }

                    if save_and_repaint {
                        st.save_layouts();
                        st.sync_adaptive_timers();
                        if weather_changed {
                            st.trigger_weather_refresh(true);
                        }
                        if let Some(ww) = st.widget_windows.get(&wid) {
                            ww.invalidate();
                        }
                        if let Some(ew) = st.edit_win.as_ref() {
                            ew.invalidate();
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            if let Some(app) = get_global_app() {
                if vk == VK_ESCAPE.0 {
                    enqueue_cmd(AppCommand::CloseEditWidget);
                    return LRESULT(0);
                }
                let mut st = app.borrow_mut();
                if let Some(ew) = st.edit_win.as_mut()
                    && ew.city_input_focused
                {
                    if vk == VK_BACK.0 {
                        ew.city_query.pop();
                        ew.invalidate();
                        return LRESULT(0);
                    }
                    if vk == VK_RETURN.0 {
                        let q = ew.city_query.clone();
                        ew.city_status = Some("Searching…".to_string());
                        ew.invalidate();
                        drop(st);
                        enqueue_cmd(AppCommand::SearchWeatherCities(q));
                        return LRESULT(0);
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam.0 as u32)
                && !ch.is_control()
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                if let Some(ew) = st.edit_win.as_mut()
                    && ew.city_input_focused
                {
                    ew.city_query.push(ch);
                    ew.invalidate();
                    return LRESULT(0);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            enqueue_cmd(AppCommand::CloseEditWidget);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
