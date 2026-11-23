use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_BACK, VK_CONTROL, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics,
    IDC_ARROW, LoadCursorW, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE, SW_SHOW,
    SetForegroundWindow, ShowWindow, WM_CHAR, WM_CLOSE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows_core::PCWSTR;

use crate::core_rs;
use crate::i18n::{SUPPORTED_LANGUAGES, tr};
use crate::icons;
use crate::models::{
    LiquidGlassConfig, ResolvedTheme, Theme, WeatherConfig, WidgetLayout, resolve_theme,
};
use crate::platform::graphics::{
    WindowSurface, draw_round_rect, draw_svg_icon, draw_text, fill_circle, fill_round_rect, rect,
    rgba,
};
use crate::platform::system::{
    create_app_icon, get_clipboard_text, get_process_memory_mb, lparam_point, open_url, to_wide,
};
use crate::ui::{APP_VERSION, AppCommand, AppState, enqueue_cmd, get_global_app};
use crate::weather_client::GeoCity;
use crate::widgets::{WIDGET_CATEGORIES, render_widget_card};

const CLASS_NAME: &str = "uWidgetsRsSettings";

const ACCENT_SWATCHES: &[&str] = &[
    "#3376CD", "#0078D4", "#0099BC", "#10893E", "#FF8C00", "#E81123", "#E3008C", "#881798",
    "#767676", "#C30052",
];
const GLASS_TINT_SWATCHES: &[(&str, &str)] = &[
    ("#D8EEFF", "Ice"),
    ("#38BDF8", "Sky"),
    ("#3B82F6", "Ocean"),
    ("#8B5CF6", "Violet"),
    ("#EC4899", "Rose"),
    ("#F97316", "Sunset"),
    ("#F59E0B", "Amber"),
    ("#10B981", "Emerald"),
    ("#0F172A", "Obsidian"),
];
const FONT_FAMILIES: &[&str] = &["Inter", "Segoe UI Variable", "Segoe UI", "Microsoft YaHei"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderKind {
    Opacity,
    LgBlur,
    LgRefraction,
    LgBevelWidth,
    LgDispersion,
    LgSpecular,
    LgVibrancy,
    LgGrain,
    CornerRadius,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavPage {
    General,
    Appearance,
    Weather,
    Advanced,
    About,
    Gallery(usize),
}

#[derive(Debug, Clone)]
enum UiAction {
    SelectPage(NavPage),
    TogglePane,
    SelectLanguage(String),
    ToggleRunOnStartup,
    RestartApp,
    QuitApp,
    ApplyTemplate(usize),
    ApplyApplePreset,
    ApplyWindowsPreset,
    SetDarkMode(Option<bool>),
    SetAccentColor(Option<String>),
    DragSlider(SliderKind, D2D_RECT_F),
    ToggleMonochrome,
    EnableLiquidGlassMode,
    ApplyLgPreset(u8),
    ResetLiquidGlass,
    SetLgTint(Option<String>),
    FocusWeatherSearch,
    TriggerWeatherSearch,
    PickWeatherCity(usize),
    DetectWeatherLocation,
    RefreshWeatherNow,
    SetWeatherTempUnit(String),
    SetWeatherWindUnit(String),
    SetWeatherRefreshMin(i32),
    SetWeatherHourlyCount(i32),
    ToggleWeatherHighLow,
    ToggleWeatherCondition,
    ToggleWeatherFeelsLike,
    ToggleWeatherHumidityWind,
    ToggleWeather24H,
    ResetWeatherDefaults,
    ToggleNativeFrame,
    SetFontFamily(String),
    AdjustSize(i32),
    AdjustMargin(i32),
    AdjustRadius(i32),
    ToggleSnapPosition,
    ToggleLockPosition,
    ToggleSnapSize,
    ToggleLockSize,
    SpawnGallery {
        widget_type: String,
        sub_type: String,
        cols: i64,
        rows: i64,
    },
    OpenUrl(&'static str),
    CheckUpdates,
}

#[derive(Debug, Clone)]
struct HitBox {
    rect: D2D_RECT_F,
    action: UiAction,
}

pub struct SettingsWindow {
    pub hwnd: HWND,
    pub surface: WindowSurface,
    pub page: NavPage,
    pub scroll_y: f32,
    pub max_scroll: f32,
    pub pane_override: Option<bool>,
    pub uwu_easter_egg: bool,
    easter_buf: String,
    wallpaper: Option<ID2D1Bitmap>,
    hits: Vec<HitBox>,
    dragging_slider: Option<(SliderKind, D2D_RECT_F)>,
    pub weather_query: String,
    pub weather_input_focused: bool,
    pub weather_results: Vec<GeoCity>,
    pub weather_status: Option<String>,
}

impl SettingsWindow {
    pub fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, false);
        }
    }
}

pub fn apply_titlebar_theme(hwnd: HWND, is_dark: bool) {
    let dark: i32 = if is_dark { 1 } else { 0 };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );
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

pub fn open_or_focus(app: &Rc<RefCell<AppState>>) {
    let existing = { app.borrow().settings_win.as_ref().map(|s| s.hwnd) };
    if let Some(hwnd) = existing {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
        return;
    }

    let (is_dark, gfx, init_city) = {
        let st = app.borrow();
        (
            st.resolved_theme().is_dark,
            Rc::clone(&st.gfx),
            st.settings.weather.default_city.clone(),
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
    let w = (980.0 * dpi_scale).round() as i32;
    let h = (680.0 * dpi_scale).round() as i32;
    let (x, y) = unsafe {
        let sx = GetSystemMetrics(SM_CXSCREEN);
        let sy = GetSystemMetrics(SM_CYSCREEN);
        (((sx - w) / 2).max(24), ((sy - h) / 2).max(24))
    };
    let class_w = to_wide(CLASS_NAME);
    let title_w = to_wide("uWidgets");

    let hwnd = unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
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
    apply_titlebar_theme(hwnd, is_dark);

    let mut rc = RECT::default();
    let dpi = unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
        (GetDpiForWindow(hwnd) as f32).max(96.0)
    };
    let cw = (rc.right - rc.left).max(1) as u32;
    let ch = (rc.bottom - rc.top).max(1) as u32;

    if let Ok(surface) = WindowSurface::new(gfx, hwnd, cw, ch, dpi) {
        let wallpaper = surface.gfx.load_wallpaper_bitmap(&surface.dc);
        app.borrow_mut().settings_win = Some(SettingsWindow {
            hwnd,
            surface,
            page: NavPage::General,
            scroll_y: 0.0,
            max_scroll: 0.0,
            pane_override: None,
            uwu_easter_egg: false,
            easter_buf: String::new(),
            wallpaper,
            hits: Vec::new(),
            dragging_slider: None,
            weather_query: init_city,
            weather_input_focused: false,
            weather_results: Vec::new(),
            weather_status: None,
        });
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            let _ = InvalidateRect(hwnd, None, false);
        }
    }
}

struct Palette {
    bg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    sidebar: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    card: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    card_border: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    fg: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    muted: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    accent: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    hover: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    active: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    is_dark: bool,
}

fn make_palette(theme: ResolvedTheme) -> Palette {
    let (ar, ag, ab) = theme.accent_rgb;
    if theme.is_dark {
        Palette {
            bg: rgba(0.11, 0.115, 0.125, 1.0),
            sidebar: rgba(0.085, 0.09, 0.10, 1.0),
            card: rgba(0.155, 0.16, 0.175, 1.0),
            card_border: rgba(1.0, 1.0, 1.0, 0.08),
            fg: rgba(0.97, 0.97, 0.98, 1.0),
            muted: rgba(0.68, 0.69, 0.72, 1.0),
            accent: rgba(ar, ag, ab, 1.0),
            hover: rgba(1.0, 1.0, 1.0, 0.07),
            active: rgba(ar, ag, ab, 0.22),
            is_dark: true,
        }
    } else {
        Palette {
            bg: rgba(0.955, 0.96, 0.97, 1.0),
            sidebar: rgba(0.92, 0.925, 0.94, 1.0),
            card: rgba(1.0, 1.0, 1.0, 1.0),
            card_border: rgba(0.0, 0.0, 0.0, 0.09),
            fg: rgba(0.10, 0.10, 0.12, 1.0),
            muted: rgba(0.42, 0.43, 0.47, 1.0),
            accent: rgba(ar, ag, ab, 1.0),
            hover: rgba(0.0, 0.0, 0.0, 0.05),
            active: rgba(ar, ag, ab, 0.16),
            is_dark: false,
        }
    }
}

fn draw_settings_card_pad(
    sw: &SettingsWindow,
    pal: &Palette,
    r: D2D_RECT_F,
    title: &str,
    subtitle: &str,
    font: &str,
    right_pad: f32,
) {
    fill_round_rect(&sw.surface.dc, r, 10.0, pal.card);
    draw_round_rect(&sw.surface.dc, r, 10.0, pal.card_border, 1.0);
    let tx = r.left + 16.0;
    let right_limit = (r.right - right_pad).max(tx + 80.0);
    if subtitle.is_empty() {
        draw_text(
            &sw.surface.gfx,
            &sw.surface.dc,
            title,
            rect(tx, r.top, right_limit, r.bottom),
            font,
            13.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            pal.fg,
            false,
        );
    } else {
        draw_text(
            &sw.surface.gfx,
            &sw.surface.dc,
            title,
            rect(tx, r.top + 8.0, right_limit, r.top + 29.0),
            font,
            13.5,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            pal.fg,
            false,
        );
        draw_text(
            &sw.surface.gfx,
            &sw.surface.dc,
            subtitle,
            rect(tx, r.top + 28.0, right_limit, r.bottom - 6.0),
            font,
            11.5,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            pal.muted,
            false,
        );
    }
}

fn draw_settings_card(
    sw: &SettingsWindow,
    pal: &Palette,
    r: D2D_RECT_F,
    title: &str,
    subtitle: &str,
    font: &str,
) {
    draw_settings_card_pad(sw, pal, r, title, subtitle, font, 150.0);
}

fn draw_toggle(
    sw: &SettingsWindow,
    pal: &Palette,
    right_x: f32,
    center_y: f32,
    on: bool,
) -> D2D_RECT_F {
    let r = rect(right_x - 44.0, center_y - 12.0, right_x, center_y + 12.0);
    let track_col = if on {
        pal.accent
    } else if pal.is_dark {
        rgba(1.0, 1.0, 1.0, 0.22)
    } else {
        rgba(0.0, 0.0, 0.0, 0.20)
    };
    fill_round_rect(&sw.surface.dc, r, 12.0, track_col);
    let knob_cx = if on { r.right - 12.0 } else { r.left + 12.0 };
    fill_circle(
        &sw.surface.dc,
        knob_cx,
        center_y,
        8.5,
        rgba(1.0, 1.0, 1.0, 0.98),
    );
    r
}

fn draw_pill_btn(
    sw: &SettingsWindow,
    pal: &Palette,
    r: D2D_RECT_F,
    label: &str,
    font: &str,
    primary: bool,
    selected: bool,
) {
    let bg = if primary || selected {
        pal.accent
    } else if pal.is_dark {
        rgba(1.0, 1.0, 1.0, 0.09)
    } else {
        rgba(0.0, 0.0, 0.0, 0.06)
    };
    fill_round_rect(&sw.surface.dc, r, 7.0, bg);
    if !primary && !selected {
        draw_round_rect(&sw.surface.dc, r, 7.0, pal.card_border, 1.0);
    }
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        label,
        r,
        font,
        12.0,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        if primary || selected {
            rgba(1.0, 1.0, 1.0, 1.0)
        } else {
            pal.fg
        },
        false,
    );
}

fn draw_section_label(
    sw: &SettingsWindow,
    pal: &Palette,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
    font: &str,
) {
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        text,
        rect(x, y, x + w, y + 24.0),
        font,
        12.5,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        pal.muted,
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_slider_card(
    sw: &SettingsWindow,
    pal: &Palette,
    r: D2D_RECT_F,
    title: &str,
    subtitle: &str,
    value_label: &str,
    frac: f32,
    font: &str,
    kind: SliderKind,
    hits: &mut Vec<HitBox>,
) {
    fill_round_rect(&sw.surface.dc, r, 10.0, pal.card);
    draw_round_rect(&sw.surface.dc, r, 10.0, pal.card_border, 1.0);
    let tx = r.left + 16.0;
    let text_right = (r.right - 236.0).max(tx + 80.0);
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        title,
        rect(tx, r.top + 8.0, text_right, r.top + 29.0),
        font,
        13.5,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        pal.fg,
        false,
    );
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        subtitle,
        rect(tx, r.top + 28.0, text_right, r.bottom - 6.0),
        font,
        11.5,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        pal.muted,
        false,
    );

    let sl_r = rect(
        r.right - 224.0,
        r.top + 16.0,
        r.right - 68.0,
        r.bottom - 16.0,
    );
    let track_y = (r.top + r.bottom) * 0.5;
    let t = frac.clamp(0.0, 1.0);
    let knob_x = sl_r.left + (sl_r.right - sl_r.left) * t;
    fill_round_rect(
        &sw.surface.dc,
        rect(sl_r.left, track_y - 3.0, sl_r.right, track_y + 3.0),
        3.0,
        rgba(pal.muted.r, pal.muted.g, pal.muted.b, 0.30),
    );
    fill_round_rect(
        &sw.surface.dc,
        rect(sl_r.left, track_y - 3.0, knob_x, track_y + 3.0),
        3.0,
        pal.accent,
    );
    fill_circle(&sw.surface.dc, knob_x, track_y, 7.5, pal.accent);
    fill_circle(
        &sw.surface.dc,
        knob_x,
        track_y,
        3.6,
        rgba(1.0, 1.0, 1.0, 0.96),
    );
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        value_label,
        rect(r.right - 62.0, r.top, r.right - 14.0, r.bottom),
        font,
        12.5,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        DWRITE_TEXT_ALIGNMENT_TRAILING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        pal.fg,
        false,
    );
    hits.push(HitBox {
        rect: sl_r,
        action: UiAction::DragSlider(kind, sl_r),
    });
}

fn lg_preset_values(idx: u8) -> (i32, f64, f64, f64, f64, f64, f64) {
    match idx {
        // 0: Crystal — ultra-clear lens glass (liquidGL style)
        0 => (4, 1.55, 1.25, 1.25, 1.55, 1.40, 0.20),
        // 1: iOS 26 — balanced Apple Liquid Glass default
        1 => (15, 1.00, 1.00, 1.00, 1.00, 1.30, 1.00),
        // 2: Deep Frost — silky studio frosted glass
        2 => (26, 0.55, 0.85, 0.35, 0.85, 1.15, 1.65),
        // 3: Prism — iridescent high-refraction RGB chromatic glass
        _ => (10, 1.80, 1.45, 2.30, 1.65, 1.60, 0.45),
    }
}

fn lg_matches_preset(lg: &LiquidGlassConfig, idx: u8) -> bool {
    let (blur, refr, bev, disp, spec, vib, grain) = lg_preset_values(idx);
    lg.blur_radius == blur
        && (lg.refraction - refr).abs() < 0.03
        && (lg.bevel_width - bev).abs() < 0.03
        && (lg.dispersion - disp).abs() < 0.03
        && (lg.specular - spec).abs() < 0.03
        && (lg.vibrancy - vib).abs() < 0.03
        && (lg.frost_grain - grain).abs() < 0.03
}

fn paint_settings(st: &mut AppState) {
    let theme = st.resolved_theme();
    let pal = make_palette(theme);
    let settings = st.settings.clone();
    let lang = settings.region.language.clone();
    let font = settings.theme.font_family.clone();
    let sys_dark = st.system_dark;
    let sys_accent = st.system_accent;
    let lang_changed = st.language_changed;
    let update_status = st.update_status.clone();

    let AppState {
        settings_win,
        monitor,
        weather,
        timezones,
        ..
    } = st;

    let Some(sw) = settings_win.as_mut() else {
        return;
    };
    let w = sw.surface.width as f32 * 96.0 / sw.surface.dpi;
    let h = sw.surface.height as f32 * 96.0 / sw.surface.dpi;

    let pane_open = sw.pane_override.unwrap_or(w >= 800.0);
    let sidebar_w = if pane_open { 224.0_f32 } else { 58.0_f32 };

    let mut hits: Vec<HitBox> = Vec::new();
    sw.surface.begin();

    // Background + Sidebar
    fill_round_rect(&sw.surface.dc, rect(0.0, 0.0, w, h), 0.0, pal.bg);
    fill_round_rect(
        &sw.surface.dc,
        rect(0.0, 0.0, sidebar_w, h),
        0.0,
        pal.sidebar,
    );
    fill_round_rect(
        &sw.surface.dc,
        rect(sidebar_w - 1.0, 0.0, sidebar_w, h),
        0.0,
        pal.card_border,
    );

    // Sidebar Header ("uWidgets" or Easter egg "UwUidgets"!)
    let toggle_r = rect(10.0, 12.0, 44.0, 42.0);
    fill_round_rect(&sw.surface.dc, toggle_r, 7.0, pal.hover);
    draw_text(
        &sw.surface.gfx,
        &sw.surface.dc,
        "☰",
        toggle_r,
        &font,
        15.0,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        pal.fg,
        false,
    );
    hits.push(HitBox {
        rect: toggle_r,
        action: UiAction::TogglePane,
    });
    if pane_open {
        let app_title = if sw.uwu_easter_egg {
            "UwUidgets"
        } else {
            "uWidgets"
        };
        draw_text(
            &sw.surface.gfx,
            &sw.surface.dc,
            app_title,
            rect(52.0, 12.0, sidebar_w - 10.0, 42.0),
            &font,
            16.5,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            pal.fg,
            false,
        );
    }

    // Sidebar Navigation Items
    let mut sy = 56.0_f32;
    if pane_open {
        draw_section_label(
            sw,
            &pal,
            14.0,
            sy,
            sidebar_w - 24.0,
            tr(&lang, "Settings"),
            &font,
        );
        sy += 24.0;
    }
    let nav_items: &[(NavPage, &str, &str)] = &[
        (NavPage::General, "Settings_General", icons::ICON_GENERAL),
        (
            NavPage::Appearance,
            "Settings_Appearance",
            icons::ICON_APPEARANCE,
        ),
        (NavPage::Weather, "Weather", icons::CAT_WEATHER),
        (NavPage::Advanced, "Settings_Advanced", icons::ICON_ADVANCED),
        (NavPage::About, "Settings_About", icons::ICON_ABOUT),
    ];
    for (page, key, svg) in nav_items {
        let item_r = rect(8.0, sy, sidebar_w - 8.0, sy + 36.0);
        let active = sw.page == *page;
        if active {
            fill_round_rect(&sw.surface.dc, item_r, 8.0, pal.active);
            fill_round_rect(
                &sw.surface.dc,
                rect(
                    item_r.left,
                    item_r.top + 8.0,
                    item_r.left + 3.5,
                    item_r.bottom - 8.0,
                ),
                2.0,
                pal.accent,
            );
        }
        draw_svg_icon(
            &sw.surface.gfx,
            &sw.surface.dc,
            svg,
            rect(
                item_r.left + 12.0,
                item_r.top + 9.0,
                item_r.left + 30.0,
                item_r.top + 27.0,
            ),
            if active { pal.accent } else { pal.muted },
        );
        if pane_open {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, key),
                rect(
                    item_r.left + 40.0,
                    item_r.top,
                    item_r.right - 8.0,
                    item_r.bottom,
                ),
                &font,
                13.0,
                if active {
                    DWRITE_FONT_WEIGHT_SEMI_BOLD
                } else {
                    DWRITE_FONT_WEIGHT_MEDIUM
                },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
        }
        hits.push(HitBox {
            rect: item_r,
            action: UiAction::SelectPage(page.clone()),
        });
        sy += 40.0;
    }

    sy += 8.0;
    if pane_open {
        draw_section_label(
            sw,
            &pal,
            14.0,
            sy,
            sidebar_w - 24.0,
            tr(&lang, "Widgets"),
            &font,
        );
        sy += 24.0;
    }
    for (ci, cat) in WIDGET_CATEGORIES.iter().enumerate() {
        let item_r = rect(8.0, sy, sidebar_w - 8.0, sy + 36.0);
        let active = sw.page == NavPage::Gallery(ci);
        if active {
            fill_round_rect(&sw.surface.dc, item_r, 8.0, pal.active);
            fill_round_rect(
                &sw.surface.dc,
                rect(
                    item_r.left,
                    item_r.top + 8.0,
                    item_r.left + 3.5,
                    item_r.bottom - 8.0,
                ),
                2.0,
                pal.accent,
            );
        }
        draw_svg_icon(
            &sw.surface.gfx,
            &sw.surface.dc,
            cat.icon_svg,
            rect(
                item_r.left + 12.0,
                item_r.top + 9.0,
                item_r.left + 30.0,
                item_r.top + 27.0,
            ),
            if active { pal.accent } else { pal.muted },
        );
        if pane_open {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, cat.locale_key),
                rect(
                    item_r.left + 40.0,
                    item_r.top,
                    item_r.right - 8.0,
                    item_r.bottom,
                ),
                &font,
                13.0,
                if active {
                    DWRITE_FONT_WEIGHT_SEMI_BOLD
                } else {
                    DWRITE_FONT_WEIGHT_MEDIUM
                },
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
        }
        hits.push(HitBox {
            rect: item_r,
            action: UiAction::SelectPage(NavPage::Gallery(ci)),
        });
        sy += 40.0;
    }

    // Content Viewport (clipped + scrolled)
    let content_x = sidebar_w + 28.0;
    let content_w = (w - content_x - 28.0).max(260.0);
    let content_clip = rect(sidebar_w, 0.0, w, h);
    unsafe {
        sw.surface.dc.PushAxisAlignedClip(
            &content_clip,
            windows::Win32::Graphics::Direct2D::D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        );
    }

    let mut cy = 22.0 - sw.scroll_y;
    let page = sw.page.clone();

    match page {
        NavPage::General => {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_General"),
                rect(content_x, cy, content_x + content_w, cy + 34.0),
                &font,
                24.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            cy += 46.0;

            if lang_changed {
                let banner = rect(content_x, cy, content_x + content_w, cy + 38.0);
                fill_round_rect(&sw.surface.dc, banner, 8.0, pal.active);
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    tr(&lang, "Settings_RestartRequired"),
                    rect(
                        banner.left + 14.0,
                        banner.top,
                        banner.right - 14.0,
                        banner.bottom,
                    ),
                    &font,
                    12.5,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    pal.fg,
                    false,
                );
                cy += 48.0;
            }

            // Language Section
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_General_Language"),
                &font,
            );
            cy += 28.0;

            let lang_card = rect(content_x, cy, content_x + content_w, cy + 128.0);
            fill_round_rect(&sw.surface.dc, lang_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, lang_card, 10.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_General_Language_Title"),
                rect(
                    content_x + 16.0,
                    cy + 10.0,
                    content_x + content_w - 16.0,
                    cy + 30.0,
                ),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_General_Language_Subtitle"),
                rect(
                    content_x + 16.0,
                    cy + 30.0,
                    content_x + content_w - 16.0,
                    cy + 48.0,
                ),
                &font,
                11.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                false,
            );
            // Language pills grid (6 per row)
            let pill_cols = 6usize;
            let pill_gap = 6.0_f32;
            let pill_w =
                (content_w - 32.0 - pill_gap * (pill_cols as f32 - 1.0)) / pill_cols as f32;
            for (li, (code, label)) in SUPPORTED_LANGUAGES.iter().enumerate() {
                let row = li / pill_cols;
                let col = li % pill_cols;
                let px = content_x + 16.0 + col as f32 * (pill_w + pill_gap);
                let py = cy + 56.0 + row as f32 * 32.0;
                let pr = rect(px, py, px + pill_w, py + 26.0);
                draw_pill_btn(sw, &pal, pr, label, &font, false, *code == lang);
                hits.push(HitBox {
                    rect: pr,
                    action: UiAction::SelectLanguage((*code).to_string()),
                });
            }
            cy += 144.0;

            // Startup Section
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_General_Startup"),
                &font,
            );
            cy += 28.0;
            let st_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card(
                sw,
                &pal,
                st_card,
                tr(&lang, "Settings_General_Startup_Title"),
                tr(&lang, "Settings_General_Startup_Subtitle"),
                &font,
            );
            let tr_tog = draw_toggle(
                sw,
                &pal,
                st_card.right - 16.0,
                cy + 31.0,
                settings.run_on_startup,
            );
            hits.push(HitBox {
                rect: tr_tog,
                action: UiAction::ToggleRunOnStartup,
            });
            cy += 78.0;

            // Actions Section (Restart & Exit)
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_General_Actions"),
                &font,
            );
            cy += 28.0;
            let r_card = rect(content_x, cy, content_x + content_w, cy + 56.0);
            draw_settings_card(
                sw,
                &pal,
                r_card,
                tr(&lang, "Settings_General_Restart_Title"),
                "",
                &font,
            );
            let r_btn = rect(
                r_card.right - 108.0,
                cy + 12.0,
                r_card.right - 16.0,
                cy + 44.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                r_btn,
                tr(&lang, "Settings_General_Restart_Action"),
                &font,
                false,
                false,
            );
            hits.push(HitBox {
                rect: r_btn,
                action: UiAction::RestartApp,
            });
            cy += 64.0;

            let e_card = rect(content_x, cy, content_x + content_w, cy + 56.0);
            draw_settings_card(
                sw,
                &pal,
                e_card,
                tr(&lang, "Settings_General_Exit_Title"),
                "",
                &font,
            );
            let e_btn = rect(
                e_card.right - 108.0,
                cy + 12.0,
                e_card.right - 16.0,
                cy + 44.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                e_btn,
                tr(&lang, "Settings_General_Exit_Action"),
                &font,
                false,
                false,
            );
            hits.push(HitBox {
                rect: e_btn,
                action: UiAction::QuitApp,
            });
            cy += 76.0;
        }
        NavPage::Appearance => {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_Appearance"),
                rect(content_x, cy, content_x + content_w, cy + 34.0),
                &font,
                24.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            cy += 46.0;

            // Theme Templates Banner (with desktop wallpaper background!)
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Appearance_ApplyTheme"),
                &font,
            );
            cy += 28.0;
            let wall_r = rect(content_x, cy, content_x + content_w, cy + 132.0);
            fill_round_rect(&sw.surface.dc, wall_r, 12.0, rgba(0.16, 0.24, 0.36, 1.0));
            if let Some(bmp) = &sw.wallpaper {
                unsafe {
                    sw.surface.dc.DrawBitmap(
                        bmp,
                        Some(&wall_r),
                        1.0,
                        windows::Win32::Graphics::Direct2D::D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                }
            }
            draw_round_rect(&sw.surface.dc, wall_r, 12.0, pal.card_border, 1.0);

            // 8 ThemeButton template cards
            let tpl_count = settings.templates.len().min(8);
            if tpl_count > 0 {
                let gap = 10.0_f32;
                let pad_x = 14.0_f32;
                let card_w = ((content_w - pad_x * 2.0 - gap * (tpl_count as f32 - 1.0))
                    / tpl_count as f32)
                    .clamp(48.0, 88.0);
                let card_h = 84.0_f32;
                let start_y = cy + 24.0;
                for (ti, tpl) in settings.templates.iter().take(8).enumerate() {
                    let tx = content_x + pad_x + ti as f32 * (card_w + gap);
                    let tr_r = rect(tx, start_y, tx + card_w, start_y + card_h);
                    let rt = resolve_theme(tpl, &settings.dimensions, sys_dark, sys_accent);
                    let preview_rad = core_rs::port::theme_preview_radius(tpl.use_native_frame)
                        .unwrap_or(6) as f32;
                    let preview_bdr = core_rs::port::theme_preview_border(tpl.use_native_frame)
                        .unwrap_or(0) as f32;
                    fill_round_rect(
                        &sw.surface.dc,
                        tr_r,
                        preview_rad,
                        rgba(rt.bg_rgb.0, rt.bg_rgb.1, rt.bg_rgb.2, rt.bg_alpha),
                    );
                    if preview_bdr > 0.0 {
                        draw_round_rect(
                            &sw.surface.dc,
                            tr_r,
                            preview_rad,
                            rgba(1.0, 1.0, 1.0, 0.35),
                            preview_bdr,
                        );
                    }
                    let is_match = settings.theme.dark_mode == tpl.dark_mode
                        && (settings.theme.opacity_level - tpl.opacity_level).abs() < 0.05
                        && settings.theme.use_native_frame == tpl.use_native_frame;
                    if is_match {
                        draw_round_rect(
                            &sw.surface.dc,
                            rect(
                                tr_r.left - 2.0,
                                tr_r.top - 2.0,
                                tr_r.right + 2.0,
                                tr_r.bottom + 2.0,
                            ),
                            preview_rad + 2.0,
                            pal.accent,
                            2.5,
                        );
                    }
                    draw_text(
                        &sw.surface.gfx,
                        &sw.surface.dc,
                        "23",
                        tr_r,
                        &tpl.font_family,
                        22.0,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                        rgba(rt.fg_rgb.0, rt.fg_rgb.1, rt.fg_rgb.2, rt.text_opacity),
                        false,
                    );
                    hits.push(HitBox {
                        rect: tr_r,
                        action: UiAction::ApplyTemplate(ti),
                    });
                }
            }
            cy += 146.0;

            // Apple & Windows Quick Presets
            let half_w = (content_w - 12.0) * 0.5;
            for (idx, (icon, title_k, sub_k, action)) in [
                (
                    icons::ICON_APPLE,
                    "Settings_Appearance_Presets_Apple_Title",
                    "Settings_Appearance_Presets_Apple_Subtitle",
                    UiAction::ApplyApplePreset,
                ),
                (
                    icons::ICON_WINDOWS,
                    "Settings_Appearance_Presets_Windows_Title",
                    "Settings_Appearance_Presets_Windows_Subtitle",
                    UiAction::ApplyWindowsPreset,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let px = content_x + idx as f32 * (half_w + 12.0);
                let pr = rect(px, cy, px + half_w, cy + 68.0);
                fill_round_rect(&sw.surface.dc, pr, 10.0, pal.card);
                draw_round_rect(&sw.surface.dc, pr, 10.0, pal.card_border, 1.0);
                draw_svg_icon(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    icon,
                    rect(px + 14.0, cy + 20.0, px + 38.0, cy + 44.0),
                    pal.fg,
                );
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    tr(&lang, title_k),
                    rect(px + 48.0, cy + 10.0, pr.right - 84.0, cy + 32.0),
                    &font,
                    13.5,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    pal.fg,
                    false,
                );
                let sub_text = if idx == 0 && lang == "en" {
                    "Inter, Liquid Glass frost & refraction, round corners"
                } else if idx == 1 && lang == "en" {
                    "Segoe UI Variable, native Win11 Acrylic frame"
                } else {
                    tr(&lang, sub_k)
                };
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    sub_text,
                    rect(px + 48.0, cy + 32.0, pr.right - 84.0, cy + 58.0),
                    &font,
                    11.0,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    pal.muted,
                    true,
                );
                let apply_r = rect(pr.right - 76.0, cy + 19.0, pr.right - 12.0, cy + 49.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    apply_r,
                    tr(&lang, "Settings_Appearance_Presets_Apply"),
                    &font,
                    false,
                    false,
                );
                hits.push(HitBox {
                    rect: apply_r,
                    action,
                });
            }
            cy += 84.0;

            // Colors Section
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Appearance_Colors"),
                &font,
            );
            cy += 28.0;

            // Dark Mode segmented control
            let dm_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card(
                sw,
                &pal,
                dm_card,
                tr(&lang, "Settings_Appearance_DarkMode_Title"),
                tr(&lang, "Settings_Appearance_DarkMode_Subtitle"),
                &font,
            );
            let seg_opts = [
                (Some(false), tr(&lang, "Settings_Appearance_DarkMode_False")),
                (
                    Some(true),
                    tr(&lang, "Settings_Appearance_DarkMode_True").trim(),
                ),
                (None, tr(&lang, "Settings_Appearance_DarkMode_Null")),
            ];
            for (si, (val, lbl)) in seg_opts.iter().enumerate() {
                let bx = dm_card.right - 228.0 + si as f32 * 72.0;
                let br = rect(bx, cy + 16.0, bx + 66.0, cy + 46.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    br,
                    lbl,
                    &font,
                    false,
                    settings.theme.dark_mode == *val,
                );
                hits.push(HitBox {
                    rect: br,
                    action: UiAction::SetDarkMode(*val),
                });
            }
            cy += 70.0;

            // Accent Color + Swatches
            let ac_card = rect(content_x, cy, content_x + content_w, cy + 98.0);
            fill_round_rect(&sw.surface.dc, ac_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, ac_card, 10.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_Appearance_AccentColor_Title"),
                rect(content_x + 16.0, cy + 12.0, content_x + 260.0, cy + 36.0),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            let sys_btn = rect(
                ac_card.right - 168.0,
                cy + 12.0,
                ac_card.right - 96.0,
                cy + 40.0,
            );
            let man_btn = rect(
                ac_card.right - 90.0,
                cy + 12.0,
                ac_card.right - 16.0,
                cy + 40.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                sys_btn,
                tr(&lang, "Settings_Appearance_AccentColor_Null"),
                &font,
                false,
                settings.theme.accent_color.is_none(),
            );
            draw_pill_btn(
                sw,
                &pal,
                man_btn,
                tr(&lang, "Settings_Appearance_AccentColor_Manual"),
                &font,
                false,
                settings.theme.accent_color.is_some(),
            );
            hits.push(HitBox {
                rect: sys_btn,
                action: UiAction::SetAccentColor(None),
            });
            hits.push(HitBox {
                rect: man_btn,
                action: UiAction::SetAccentColor(Some(
                    settings
                        .theme
                        .accent_color
                        .clone()
                        .unwrap_or_else(|| "#3376CD".to_string()),
                )),
            });

            // 10 clickable accent swatches
            for (si, hex) in ACCENT_SWATCHES.iter().enumerate() {
                let cx = content_x + 28.0 + si as f32 * 34.0;
                let scy = cy + 70.0;
                if let Some((r, g, b)) = crate::models::parse_hex_color(hex) {
                    fill_circle(&sw.surface.dc, cx, scy, 11.0, rgba(r, g, b, 1.0));
                    let sel = settings
                        .theme
                        .accent_color
                        .as_deref()
                        .is_some_and(|c| c.eq_ignore_ascii_case(hex));
                    if sel {
                        crate::platform::graphics::draw_circle(
                            &sw.surface.dc,
                            cx,
                            scy,
                            14.0,
                            pal.fg,
                            2.0,
                        );
                    }
                    hits.push(HitBox {
                        rect: rect(cx - 14.0, scy - 14.0, cx + 14.0, scy + 14.0),
                        action: UiAction::SetAccentColor(Some((*hex).to_string())),
                    });
                }
            }
            cy += 108.0;

            // Opacity Level Slider
            let op_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_slider_card(
                sw,
                &pal,
                op_card,
                tr(&lang, "Settings_Appearance_OpacityLevel_Title"),
                tr(&lang, "Settings_Appearance_OpacityLevel_Subtitle"),
                &format!("{}%", (settings.theme.opacity_level * 100.0).round() as i32),
                ((settings.theme.opacity_level - 0.05) / 0.95).clamp(0.0, 1.0) as f32,
                &font,
                SliderKind::Opacity,
                &mut hits,
            );
            cy += 70.0;

            // Monochrome Toggle
            let mono_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card(
                sw,
                &pal,
                mono_card,
                tr(&lang, "Settings_Appearance_Monochrome_Title"),
                tr(&lang, "Settings_Appearance_Monochrome_Subtitle"),
                &font,
            );
            let m_tog = draw_toggle(
                sw,
                &pal,
                mono_card.right - 16.0,
                cy + 31.0,
                settings.theme.monochrome,
            );
            hits.push(HitBox {
                rect: m_tog,
                action: UiAction::ToggleMonochrome,
            });
            cy += 80.0;

            // Liquid Glass Customization Section
            draw_section_label(sw, &pal, content_x, cy, content_w, "Liquid Glass", &font);
            cy += 28.0;

            let lg = &settings.liquid_glass;
            let presets_card = rect(content_x, cy, content_x + content_w, cy + 104.0);
            fill_round_rect(&sw.surface.dc, presets_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, presets_card, 10.0, pal.card_border, 1.0);

            let reset_btn = rect(
                presets_card.right - 80.0,
                cy + 12.0,
                presets_card.right - 16.0,
                cy + 40.0,
            );
            let enable_btn_right = if settings.theme.use_native_frame {
                let enable_btn = rect(
                    reset_btn.left - 86.0,
                    cy + 12.0,
                    reset_btn.left - 8.0,
                    cy + 40.0,
                );
                draw_pill_btn(sw, &pal, enable_btn, "Enable", &font, true, false);
                hits.push(HitBox {
                    rect: enable_btn,
                    action: UiAction::EnableLiquidGlassMode,
                });
                enable_btn.left - 10.0
            } else {
                reset_btn.left - 10.0
            };
            draw_pill_btn(sw, &pal, reset_btn, "Reset", &font, false, false);
            hits.push(HitBox {
                rect: reset_btn,
                action: UiAction::ResetLiquidGlass,
            });

            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "Optical Presets",
                rect(content_x + 16.0, cy + 10.0, enable_btn_right, cy + 30.0),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            let lg_status_sub = if settings.theme.use_native_frame {
                "Windows Acrylic active — click Enable or a preset for curved refraction"
            } else if settings.theme.opacity_level >= 0.995 {
                "Opacity is 100% (solid) — lower Opacity above to reveal Liquid Glass"
            } else {
                "Real-time SDF refraction, Gaussian frost & dual-angle Fresnel specular"
            };
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                lg_status_sub,
                rect(content_x + 16.0, cy + 30.0, enable_btn_right, cy + 48.0),
                &font,
                11.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                false,
            );

            let preset_labels = ["Crystal", "iOS 26", "Deep Frost", "Prism"];
            let p_gap = 8.0_f32;
            let p_btn_w = ((content_w - 32.0 - p_gap * 3.0) / 4.0).max(70.0);
            for (pi, plbl) in preset_labels.iter().enumerate() {
                let px = content_x + 16.0 + pi as f32 * (p_btn_w + p_gap);
                let pr = rect(px, cy + 60.0, px + p_btn_w, cy + 90.0);
                let active = !settings.theme.use_native_frame && lg_matches_preset(lg, pi as u8);
                draw_pill_btn(sw, &pal, pr, plbl, &font, false, active);
                hits.push(HitBox {
                    rect: pr,
                    action: UiAction::ApplyLgPreset(pi as u8),
                });
            }
            cy += 112.0;

            // 8 Live Liquid Glass Optical Sliders
            let lg_sliders: [(&str, &str, String, f32, SliderKind); 8] = [
                (
                    "Blur Radius",
                    "Frosted Gaussian blur softness over desktop wallpaper",
                    format!("{} px", lg.blur_radius.clamp(0, 30)),
                    (lg.blur_radius.clamp(0, 30) as f32) / 30.0,
                    SliderKind::LgBlur,
                ),
                (
                    "Refraction",
                    "Curved glass lens bend strength along card edges",
                    format!(
                        "{}%",
                        (lg.refraction.clamp(0.0, 2.0) * 100.0).round() as i32
                    ),
                    (lg.refraction.clamp(0.0, 2.0) / 2.0) as f32,
                    SliderKind::LgRefraction,
                ),
                (
                    "Bevel Width",
                    "Depth of the curved convex optical glass rim",
                    format!(
                        "{}%",
                        (lg.bevel_width.clamp(0.3, 1.8) * 100.0).round() as i32
                    ),
                    ((lg.bevel_width.clamp(0.3, 1.8) - 0.3) / 1.5) as f32,
                    SliderKind::LgBevelWidth,
                ),
                (
                    "Chromatic Dispersion",
                    "Prismatic RGB color splitting along curved glass",
                    format!(
                        "{}%",
                        (lg.dispersion.clamp(0.0, 2.5) * 100.0).round() as i32
                    ),
                    (lg.dispersion.clamp(0.0, 2.5) / 2.5) as f32,
                    SliderKind::LgDispersion,
                ),
                (
                    "Specular Rim",
                    "Dual-angle Fresnel edge gloss & dome highlight",
                    format!("{}%", (lg.specular.clamp(0.0, 2.0) * 100.0).round() as i32),
                    (lg.specular.clamp(0.0, 2.0) / 2.0) as f32,
                    SliderKind::LgSpecular,
                ),
                (
                    "Vibrancy",
                    "Backdrop chroma saturation boost through glass",
                    format!("{}%", (lg.vibrancy.clamp(0.5, 2.0) * 100.0).round() as i32),
                    ((lg.vibrancy.clamp(0.5, 2.0) - 0.5) / 1.5) as f32,
                    SliderKind::LgVibrancy,
                ),
                (
                    "Frost Grain",
                    "Subtle frosted acrylic micro-texture & dithering",
                    format!(
                        "{}%",
                        (lg.frost_grain.clamp(0.0, 2.5) * 100.0).round() as i32
                    ),
                    (lg.frost_grain.clamp(0.0, 2.5) / 2.5) as f32,
                    SliderKind::LgGrain,
                ),
                (
                    "Corner Radius",
                    if settings.theme.use_native_frame {
                        "Squircle roundness (applies in Liquid Glass mode)"
                    } else {
                        "Squircle curvature of Liquid Glass widget cards"
                    },
                    format!("{} px", settings.dimensions.radius.clamp(0, 44)),
                    (settings.dimensions.radius.clamp(0, 44) as f32) / 44.0,
                    SliderKind::CornerRadius,
                ),
            ];
            for (title, sub, val_lbl, frac, kind) in lg_sliders {
                let c = rect(content_x, cy, content_x + content_w, cy + 60.0);
                draw_slider_card(
                    sw, &pal, c, title, sub, &val_lbl, frac, &font, kind, &mut hits,
                );
                cy += 66.0;
            }

            // Glass Tint Dye Swatch Card
            let tint_card = rect(content_x, cy, content_x + content_w, cy + 114.0);
            fill_round_rect(&sw.surface.dc, tint_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, tint_card, 10.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "Glass Tint",
                rect(
                    content_x + 16.0,
                    cy + 10.0,
                    tint_card.right - 96.0,
                    cy + 30.0,
                ),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "Optional translucent dye tint blended into frosted glass",
                rect(
                    content_x + 16.0,
                    cy + 30.0,
                    tint_card.right - 96.0,
                    cy + 48.0,
                ),
                &font,
                11.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                false,
            );
            let auto_tint_btn = rect(
                tint_card.right - 88.0,
                cy + 12.0,
                tint_card.right - 16.0,
                cy + 40.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                auto_tint_btn,
                "Auto",
                &font,
                false,
                lg.tint_color.is_none(),
            );
            hits.push(HitBox {
                rect: auto_tint_btn,
                action: UiAction::SetLgTint(None),
            });
            let sw_count = GLASS_TINT_SWATCHES.len() as f32;
            let sw_slot_w = ((content_w - 32.0) / sw_count).max(34.0);
            for (si, (hex, name)) in GLASS_TINT_SWATCHES.iter().enumerate() {
                let cx = content_x + 16.0 + (si as f32 + 0.5) * sw_slot_w;
                let scy = cy + 70.0;
                if let Some((r, g, b)) = crate::models::parse_hex_color(hex) {
                    fill_circle(&sw.surface.dc, cx, scy, 11.5, rgba(r, g, b, 1.0));
                    crate::platform::graphics::draw_circle(
                        &sw.surface.dc,
                        cx,
                        scy,
                        11.5,
                        if pal.is_dark {
                            rgba(1.0, 1.0, 1.0, 0.28)
                        } else {
                            rgba(0.0, 0.0, 0.0, 0.20)
                        },
                        1.0,
                    );
                    let sel = lg
                        .tint_color
                        .as_deref()
                        .is_some_and(|c| c.eq_ignore_ascii_case(hex));
                    if sel {
                        crate::platform::graphics::draw_circle(
                            &sw.surface.dc,
                            cx,
                            scy,
                            14.5,
                            pal.fg,
                            2.0,
                        );
                    }
                    draw_text(
                        &sw.surface.gfx,
                        &sw.surface.dc,
                        name,
                        rect(
                            cx - sw_slot_w * 0.5,
                            cy + 87.0,
                            cx + sw_slot_w * 0.5,
                            cy + 106.0,
                        ),
                        &font,
                        10.0,
                        if sel {
                            DWRITE_FONT_WEIGHT_SEMI_BOLD
                        } else {
                            DWRITE_FONT_WEIGHT_NORMAL
                        },
                        DWRITE_TEXT_ALIGNMENT_CENTER,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                        if sel { pal.fg } else { pal.muted },
                        false,
                    );
                    hits.push(HitBox {
                        rect: rect(
                            cx - sw_slot_w * 0.45,
                            cy + 54.0,
                            cx + sw_slot_w * 0.45,
                            cy + 106.0,
                        ),
                        action: UiAction::SetLgTint(Some((*hex).to_string())),
                    });
                }
            }
            cy += 130.0;
        }
        NavPage::Weather => {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Weather"),
                rect(content_x, cy, content_x + content_w - 90.0, cy + 34.0),
                &font,
                24.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            let reset_w_btn = rect(
                content_x + content_w - 78.0,
                cy + 3.0,
                content_x + content_w,
                cy + 31.0,
            );
            draw_pill_btn(sw, &pal, reset_w_btn, "Reset", &font, false, false);
            hits.push(HitBox {
                rect: reset_w_btn,
                action: UiAction::ResetWeatherDefaults,
            });
            cy += 46.0;

            let wc = &settings.weather;
            let active_model = wc.to_forecast_model();
            let cached_entry = weather.peek_entry(&active_model);

            // Location & Live Status Section
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                "Location & Live Data",
                &font,
            );
            cy += 28.0;

            let status_card = rect(content_x, cy, content_x + content_w, cy + 68.0);
            let loc_title = if wc.latitude.abs() < 1e-6 && wc.longitude.abs() < 1e-6 {
                if let Some(ref e) = cached_entry
                    && (e.resolved_lat.abs() > 1e-6 || e.resolved_lon.abs() > 1e-6)
                {
                    format!(
                        "{}  ({:.2}, {:.2})",
                        wc.default_city, e.resolved_lat, e.resolved_lon
                    )
                } else {
                    wc.default_city.clone()
                }
            } else {
                format!(
                    "{}  ({:.2}, {:.2})",
                    wc.default_city, wc.latitude, wc.longitude
                )
            };
            let live_sub = match cached_entry {
                Some(ref e) if e.in_flight && e.forecast.is_none() => {
                    "Fetching live Open-Meteo forecast…".to_string()
                }
                Some(ref e) if let Some(ref fc) = e.forecast => {
                    let temp_s = fc
                        .current
                        .as_ref()
                        .map(|c| format!("{:.0}°", c.temperature_2m.round()))
                        .unwrap_or_else(|| "--°".to_string());
                    let age_s = e.fetched_at.elapsed().as_secs();
                    let age_lbl = if e.in_flight {
                        "Refreshing…".to_string()
                    } else if age_s < 60 {
                        "Updated just now".to_string()
                    } else {
                        format!("Updated {}m ago", age_s / 60)
                    };
                    format!("Live: {temp_s} · {age_lbl} · Disk-cached for instant launch")
                }
                Some(ref e) if let Some(ref err) = e.last_error => format!("Last error: {err}"),
                _ => "Ready — click Refresh Now or pick a city below".to_string(),
            };
            draw_settings_card_pad(sw, &pal, status_card, &loc_title, &live_sub, &font, 226.0);
            let detect_btn = rect(
                status_card.right - 216.0,
                cy + 18.0,
                status_card.right - 120.0,
                cy + 50.0,
            );
            let refresh_btn = rect(
                status_card.right - 112.0,
                cy + 18.0,
                status_card.right - 16.0,
                cy + 50.0,
            );
            draw_pill_btn(sw, &pal, detect_btn, "Auto-Detect", &font, false, false);
            draw_pill_btn(sw, &pal, refresh_btn, "Refresh Now", &font, true, false);
            hits.push(HitBox {
                rect: detect_btn,
                action: UiAction::DetectWeatherLocation,
            });
            hits.push(HitBox {
                rect: refresh_btn,
                action: UiAction::RefreshWeatherNow,
            });
            cy += 76.0;

            // Interactive City Search Card
            let visible_results = sw.weather_results.len().min(5);
            let has_status = sw.weather_status.is_some();
            let search_card_h =
                104.0 + if has_status { 22.0 } else { 0.0 } + (visible_results as f32 * 38.0);
            let search_card = rect(content_x, cy, content_x + content_w, cy + search_card_h);
            fill_round_rect(&sw.surface.dc, search_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, search_card, 10.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "Default City Search",
                rect(
                    content_x + 16.0,
                    cy + 10.0,
                    content_x + content_w - 16.0,
                    cy + 30.0,
                ),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "Updates all active Weather widgets and sets default location for new widgets",
                rect(
                    content_x + 16.0,
                    cy + 30.0,
                    content_x + content_w - 16.0,
                    cy + 48.0,
                ),
                &font,
                11.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                false,
            );

            let inp_r = rect(
                content_x + 16.0,
                cy + 56.0,
                search_card.right - 110.0,
                cy + 90.0,
            );
            fill_round_rect(
                &sw.surface.dc,
                inp_r,
                7.0,
                if pal.is_dark {
                    rgba(1.0, 1.0, 1.0, 0.07)
                } else {
                    rgba(0.0, 0.0, 0.0, 0.04)
                },
            );
            draw_round_rect(
                &sw.surface.dc,
                inp_r,
                7.0,
                if sw.weather_input_focused {
                    pal.accent
                } else {
                    pal.card_border
                },
                if sw.weather_input_focused { 1.6 } else { 1.0 },
            );
            let q_disp = if sw.weather_input_focused {
                format!("{}|", sw.weather_query)
            } else if sw.weather_query.is_empty() {
                "Click to type a city name (e.g. Dhaka, London, Tokyo)…".to_string()
            } else {
                sw.weather_query.clone()
            };
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                &q_disp,
                rect(
                    inp_r.left + 12.0,
                    inp_r.top,
                    inp_r.right - 10.0,
                    inp_r.bottom,
                ),
                &font,
                12.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                if sw.weather_query.is_empty() && !sw.weather_input_focused {
                    pal.muted
                } else {
                    pal.fg
                },
                false,
            );
            hits.push(HitBox {
                rect: inp_r,
                action: UiAction::FocusWeatherSearch,
            });

            let search_btn = rect(
                search_card.right - 102.0,
                cy + 56.0,
                search_card.right - 16.0,
                cy + 90.0,
            );
            draw_pill_btn(sw, &pal, search_btn, "Search", &font, true, false);
            hits.push(HitBox {
                rect: search_btn,
                action: UiAction::TriggerWeatherSearch,
            });

            let mut ry = cy + 96.0;
            if let Some(ref status) = sw.weather_status {
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    status,
                    rect(content_x + 18.0, ry, search_card.right - 16.0, ry + 20.0),
                    &font,
                    11.5,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    pal.muted,
                    false,
                );
                ry += 22.0;
            }
            for (ci, city) in sw.weather_results.iter().take(5).enumerate() {
                let rr = rect(
                    content_x + 16.0,
                    ry + 2.0,
                    search_card.right - 16.0,
                    ry + 34.0,
                );
                draw_pill_btn(
                    sw,
                    &pal,
                    rr,
                    &format!(
                        "{}  ({:.2}, {:.2})",
                        city.search_name(),
                        city.latitude,
                        city.longitude
                    ),
                    &font,
                    false,
                    false,
                );
                hits.push(HitBox {
                    rect: rr,
                    action: UiAction::PickWeatherCity(ci),
                });
                ry += 38.0;
            }
            cy += search_card_h + 14.0;

            // Units & Refresh Section
            draw_section_label(sw, &pal, content_x, cy, content_w, "Units & Refresh", &font);
            cy += 28.0;

            // Temperature Unit
            let tu_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card_pad(
                sw,
                &pal,
                tu_card,
                tr(&lang, "Weather_Units"),
                "Temperature scale for all Weather widgets",
                &font,
                238.0,
            );
            for (ui, (code, lbl)) in [
                ("celsius", "Celsius (°C)"),
                ("fahrenheit", "Fahrenheit (°F)"),
            ]
            .iter()
            .enumerate()
            {
                let bx = tu_card.right - 228.0 + ui as f32 * 108.0;
                let br = rect(bx, cy + 16.0, bx + 102.0, cy + 46.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    br,
                    lbl,
                    &font,
                    false,
                    wc.temperature_unit.eq_ignore_ascii_case(code),
                );
                hits.push(HitBox {
                    rect: br,
                    action: UiAction::SetWeatherTempUnit((*code).to_string()),
                });
            }
            cy += 70.0;

            // Wind Speed Unit
            let wu_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card_pad(
                sw,
                &pal,
                wu_card,
                "Wind Speed Unit",
                "Unit used when Humidity & Wind readout is enabled",
                &font,
                224.0,
            );
            for (wi, (code, lbl)) in [("kmh", "km/h"), ("mph", "mph"), ("ms", "m/s")]
                .iter()
                .enumerate()
            {
                let bx = wu_card.right - 216.0 + wi as f32 * 68.0;
                let br = rect(bx, cy + 16.0, bx + 62.0, cy + 46.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    br,
                    lbl,
                    &font,
                    false,
                    wc.wind_unit.eq_ignore_ascii_case(code),
                );
                hits.push(HitBox {
                    rect: br,
                    action: UiAction::SetWeatherWindUnit((*code).to_string()),
                });
            }
            cy += 70.0;

            // Auto-Refresh Interval
            let ri_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card_pad(
                sw,
                &pal,
                ri_card,
                "Refresh Interval",
                "How frequently background weather updates run",
                &font,
                244.0,
            );
            for (ii, (mins, lbl)) in [(5, "5m"), (15, "15m"), (30, "30m"), (60, "60m")]
                .iter()
                .enumerate()
            {
                let bx = ri_card.right - 236.0 + ii as f32 * 56.0;
                let br = rect(bx, cy + 16.0, bx + 50.0, cy + 46.0);
                draw_pill_btn(sw, &pal, br, lbl, &font, false, wc.refresh_minutes == *mins);
                hits.push(HitBox {
                    rect: br,
                    action: UiAction::SetWeatherRefreshMin(*mins),
                });
            }
            cy += 80.0;

            // Forecast Display Section
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                "Forecast Display",
                &font,
            );
            cy += 28.0;

            // Hourly Slots
            let hs_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card_pad(
                sw,
                &pal,
                hs_card,
                "Hourly Forecast Slots",
                "Number of upcoming hours shown on Wide & Large widgets",
                &font,
                224.0,
            );
            for (hi, slots) in [4, 5, 6, 8].iter().enumerate() {
                let bx = hs_card.right - 216.0 + hi as f32 * 51.0;
                let br = rect(bx, cy + 16.0, bx + 45.0, cy + 46.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    br,
                    &format!("{slots}h"),
                    &font,
                    false,
                    wc.hourly_count == *slots,
                );
                hits.push(HitBox {
                    rect: br,
                    action: UiAction::SetWeatherHourlyCount(*slots),
                });
            }
            cy += 70.0;

            for (title, sub, val, act) in [
                (
                    "Show High & Low",
                    "Display today's high and low temperature range (H / L)",
                    wc.show_high_low,
                    UiAction::ToggleWeatherHighLow,
                ),
                (
                    "Show Condition Label",
                    "Display current sky condition (Clear, Cloudy, Rain, etc.)",
                    wc.show_condition,
                    UiAction::ToggleWeatherCondition,
                ),
                (
                    "Show Feels Like",
                    "Display apparent temperature alongside current forecast",
                    wc.show_feels_like,
                    UiAction::ToggleWeatherFeelsLike,
                ),
                (
                    "Show Humidity & Wind",
                    "Display live relative humidity and wind speed metrics",
                    wc.show_humidity_wind,
                    UiAction::ToggleWeatherHumidityWind,
                ),
                (
                    "24-Hour Time Format",
                    "Use 24-hour clock for hourly forecast and sunrise/sunset",
                    wc.use_24_hour_clock,
                    UiAction::ToggleWeather24H,
                ),
            ] {
                let c = rect(content_x, cy, content_x + content_w, cy + 62.0);
                draw_settings_card(sw, &pal, c, title, sub, &font);
                let tog = draw_toggle(sw, &pal, c.right - 16.0, cy + 31.0, val);
                hits.push(HitBox {
                    rect: tog,
                    action: act,
                });
                cy += 70.0;
            }
            cy += 16.0;
        }
        NavPage::Advanced => {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_Advanced"),
                rect(content_x, cy, content_x + content_w, cy + 34.0),
                &font,
                24.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            cy += 46.0;

            // Appearance subsection: Native Frame + Font Family
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Advanced_Appearance"),
                &font,
            );
            cy += 28.0;

            let nf_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            draw_settings_card(
                sw,
                &pal,
                nf_card,
                tr(&lang, "Settings_Advanced_UseNativeFrame_Title"),
                tr(&lang, "Settings_Advanced_UseNativeFrame_Subtitle"),
                &font,
            );
            let nf_tog = draw_toggle(
                sw,
                &pal,
                nf_card.right - 16.0,
                cy + 31.0,
                settings.theme.use_native_frame,
            );
            hits.push(HitBox {
                rect: nf_tog,
                action: UiAction::ToggleNativeFrame,
            });
            cy += 70.0;

            let ff_card = rect(content_x, cy, content_x + content_w, cy + 92.0);
            fill_round_rect(&sw.surface.dc, ff_card, 10.0, pal.card);
            draw_round_rect(&sw.surface.dc, ff_card, 10.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_Advanced_FontFamily_Title"),
                rect(
                    content_x + 16.0,
                    cy + 10.0,
                    content_x + content_w - 16.0,
                    cy + 30.0,
                ),
                &font,
                13.5,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_Advanced_FontFamily_Subtitle"),
                rect(
                    content_x + 16.0,
                    cy + 29.0,
                    content_x + content_w - 16.0,
                    cy + 46.0,
                ),
                &font,
                11.5,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                false,
            );
            let f_gap = 8.0_f32;
            let f_btn_w = ((content_w - 32.0 - f_gap * (FONT_FAMILIES.len() as f32 - 1.0))
                / FONT_FAMILIES.len() as f32)
                .max(76.0);
            for (fi, fam) in FONT_FAMILIES.iter().enumerate() {
                let fx = content_x + 16.0 + fi as f32 * (f_btn_w + f_gap);
                let fr = rect(fx, cy + 54.0, fx + f_btn_w, cy + 82.0);
                draw_pill_btn(
                    sw,
                    &pal,
                    fr,
                    fam,
                    &font,
                    false,
                    settings.theme.font_family == *fam,
                );
                hits.push(HitBox {
                    rect: fr,
                    action: UiAction::SetFontFamily((*fam).to_string()),
                });
            }
            cy += 108.0;

            // Dimensions Section: Size, Margin, Radius
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Advanced_Dimensions"),
                &font,
            );
            cy += 28.0;

            for (title_k, sub_k, val, dec_act, inc_act, disabled) in [
                (
                    "Settings_Advanced_WidgetSize_Title",
                    "Settings_Advanced_WidgetSize_Subtitle",
                    settings.dimensions.size,
                    UiAction::AdjustSize(-4),
                    UiAction::AdjustSize(4),
                    false,
                ),
                (
                    "Settings_Advanced_WidgetMargin_Title",
                    "Settings_Advanced_WidgetMargin_Subtitle",
                    settings.dimensions.margin,
                    UiAction::AdjustMargin(-2),
                    UiAction::AdjustMargin(2),
                    false,
                ),
                (
                    "Settings_Advanced_WidgetRadius_Title",
                    "Settings_Advanced_WidgetRadius_Subtitle",
                    settings.dimensions.radius,
                    UiAction::AdjustRadius(-2),
                    UiAction::AdjustRadius(2),
                    settings.theme.use_native_frame,
                ),
            ] {
                let c = rect(content_x, cy, content_x + content_w, cy + 62.0);
                draw_settings_card(sw, &pal, c, tr(&lang, title_k), tr(&lang, sub_k), &font);
                let minus_r = rect(c.right - 134.0, cy + 16.0, c.right - 102.0, cy + 46.0);
                let plus_r = rect(c.right - 48.0, cy + 16.0, c.right - 16.0, cy + 46.0);
                draw_pill_btn(sw, &pal, minus_r, "−", &font, false, false);
                draw_pill_btn(sw, &pal, plus_r, "+", &font, false, false);
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    &format!("{val}"),
                    rect(c.right - 100.0, cy + 16.0, c.right - 50.0, cy + 46.0),
                    &font,
                    13.5,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    if disabled { pal.muted } else { pal.fg },
                    false,
                );
                if !disabled {
                    hits.push(HitBox {
                        rect: minus_r,
                        action: dec_act,
                    });
                    hits.push(HitBox {
                        rect: plus_r,
                        action: inc_act,
                    });
                }
                cy += 70.0;
            }

            // Position & Size Lock/Snap toggles
            cy += 8.0;
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Advanced_Position"),
                &font,
            );
            cy += 28.0;
            for (tk, sk, val, act) in [
                (
                    "Settings_Advanced_SnapPosition_Title",
                    "Settings_Advanced_SnapPosition_Subtitle",
                    settings.layout.snap_position,
                    UiAction::ToggleSnapPosition,
                ),
                (
                    "Settings_Advanced_LockPosition_Title",
                    "Settings_Advanced_LockPosition_Subtitle",
                    settings.layout.lock_position,
                    UiAction::ToggleLockPosition,
                ),
            ] {
                let c = rect(content_x, cy, content_x + content_w, cy + 62.0);
                draw_settings_card(sw, &pal, c, tr(&lang, tk), tr(&lang, sk), &font);
                let tog = draw_toggle(sw, &pal, c.right - 16.0, cy + 31.0, val);
                hits.push(HitBox {
                    rect: tog,
                    action: act,
                });
                cy += 70.0;
            }

            cy += 8.0;
            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_Advanced_Size"),
                &font,
            );
            cy += 28.0;
            for (tk, sk, val, act) in [
                (
                    "Settings_Advanced_SnapSize_Title",
                    "Settings_Advanced_SnapSize_Subtitle",
                    settings.layout.snap_size,
                    UiAction::ToggleSnapSize,
                ),
                (
                    "Settings_Advanced_LockSize_Title",
                    "Settings_Advanced_LockSize_Subtitle",
                    settings.layout.lock_size,
                    UiAction::ToggleLockSize,
                ),
            ] {
                let c = rect(content_x, cy, content_x + content_w, cy + 62.0);
                draw_settings_card(sw, &pal, c, tr(&lang, tk), tr(&lang, sk), &font);
                let tog = draw_toggle(sw, &pal, c.right - 16.0, cy + 31.0, val);
                hits.push(HitBox {
                    rect: tog,
                    action: act,
                });
                cy += 70.0;
            }
            cy += 16.0;
        }
        NavPage::About => {
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                tr(&lang, "Settings_About"),
                rect(content_x, cy, content_x + content_w, cy + 34.0),
                &font,
                24.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            cy += 48.0;

            let hero = rect(content_x, cy, content_x + content_w, cy + 106.0);
            fill_round_rect(&sw.surface.dc, hero, 12.0, pal.card);
            draw_round_rect(&sw.surface.dc, hero, 12.0, pal.card_border, 1.0);
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                "uWidgets (Rust Edition)",
                rect(
                    content_x + 20.0,
                    cy + 16.0,
                    content_x + content_w - 20.0,
                    cy + 44.0,
                ),
                &font,
                19.0,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.fg,
                false,
            );
            let (ws_mb, priv_mb) = get_process_memory_mb();
            draw_text(
                &sw.surface.gfx,
                &sw.surface.dc,
                &format!(
                    "{}: v{APP_VERSION}   •   Native Win32 + Direct2D   •   RAM: {ws_mb:.1} MB WS ({priv_mb:.1} MB Private)",
                    tr(&lang, "Settings_About_Version")
                ),
                rect(
                    content_x + 20.0,
                    cy + 48.0,
                    content_x + content_w - 20.0,
                    cy + 92.0,
                ),
                &font,
                12.5,
                DWRITE_FONT_WEIGHT_MEDIUM,
                DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                pal.muted,
                true,
            );
            cy += 124.0;

            draw_section_label(
                sw,
                &pal,
                content_x,
                cy,
                content_w,
                tr(&lang, "Settings_About_Actions"),
                &font,
            );
            cy += 28.0;

            let repo_card = rect(content_x, cy, content_x + content_w, cy + 56.0);
            draw_settings_card(
                sw,
                &pal,
                repo_card,
                tr(&lang, "Settings_About_Repository"),
                "",
                &font,
            );
            let repo_btn = rect(
                repo_card.right - 108.0,
                cy + 12.0,
                repo_card.right - 16.0,
                cy + 44.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                repo_btn,
                tr(&lang, "Settings_About_Repository_Action"),
                &font,
                false,
                false,
            );
            hits.push(HitBox {
                rect: repo_btn,
                action: UiAction::OpenUrl("https://github.com/nabil24024004/uWidgets-Rust-Rebuild"),
            });
            cy += 64.0;

            let bug_card = rect(content_x, cy, content_x + content_w, cy + 56.0);
            draw_settings_card(
                sw,
                &pal,
                bug_card,
                tr(&lang, "Settings_About_ReportBug"),
                "",
                &font,
            );
            let bug_btn = rect(
                bug_card.right - 108.0,
                cy + 12.0,
                bug_card.right - 16.0,
                cy + 44.0,
            );
            draw_pill_btn(
                sw,
                &pal,
                bug_btn,
                tr(&lang, "Settings_About_Repository_Action"),
                &font,
                false,
                false,
            );
            hits.push(HitBox {
                rect: bug_btn,
                action: UiAction::OpenUrl(
                    "https://github.com/nabil24024004/uWidgets-Rust-Rebuild/issues",
                ),
            });
            cy += 64.0;

            let upd_card = rect(content_x, cy, content_x + content_w, cy + 62.0);
            let upd_sub = update_status
                .as_deref()
                .unwrap_or("Manual check against upstream GitHub releases");
            draw_settings_card(
                sw,
                &pal,
                upd_card,
                tr(&lang, "Update_SoftwareUpdate"),
                upd_sub,
                &font,
            );
            let upd_btn = rect(
                upd_card.right - 128.0,
                cy + 15.0,
                upd_card.right - 16.0,
                cy + 47.0,
            );
            draw_pill_btn(sw, &pal, upd_btn, "Check Now", &font, true, false);
            hits.push(HitBox {
                rect: upd_btn,
                action: UiAction::CheckUpdates,
            });
            cy += 84.0;
        }
        NavPage::Gallery(cat_idx) => {
            if let Some(cat) = WIDGET_CATEGORIES.get(cat_idx) {
                draw_text(
                    &sw.surface.gfx,
                    &sw.surface.dc,
                    tr(&lang, cat.locale_key),
                    rect(content_x, cy, content_x + content_w - 160.0, cy + 34.0),
                    &font,
                    24.0,
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    pal.fg,
                    false,
                );
                if cat.widget_type == "Weather" {
                    let cfg_r = rect(
                        content_x + content_w - 154.0,
                        cy + 2.0,
                        content_x + content_w,
                        cy + 32.0,
                    );
                    draw_pill_btn(sw, &pal, cfg_r, "Weather Settings →", &font, false, false);
                    hits.push(HitBox {
                        rect: cfg_r,
                        action: UiAction::SelectPage(NavPage::Weather),
                    });
                }
                cy += 48.0;

                let mut dummy_hits = Vec::new();
                for st_meta in cat.subtypes {
                    let row_h = 196.0_f32;
                    let card_r = rect(content_x, cy, content_x + content_w, cy + row_h);
                    fill_round_rect(&sw.surface.dc, card_r, 12.0, pal.card);
                    draw_round_rect(&sw.surface.dc, card_r, 12.0, pal.card_border, 1.0);

                    // Left: Live Direct2D widget preview
                    let pv_w = if st_meta.default_cols >= 4 {
                        236.0
                    } else {
                        156.0
                    };
                    let pv_h = 156.0_f32;
                    let pv_r = rect(
                        content_x + 20.0,
                        cy + 20.0,
                        content_x + 20.0 + pv_w,
                        cy + 20.0 + pv_h,
                    );
                    // Subtle wallpaper swatch behind preview so transparency effects show clearly
                    fill_round_rect(
                        &sw.surface.dc,
                        rect(
                            pv_r.left - 6.0,
                            pv_r.top - 6.0,
                            pv_r.right + 6.0,
                            pv_r.bottom + 6.0,
                        ),
                        14.0,
                        if pal.is_dark {
                            rgba(0.18, 0.25, 0.36, 0.7)
                        } else {
                            rgba(0.76, 0.84, 0.94, 0.7)
                        },
                    );
                    let preview_layout = WidgetLayout {
                        widget_type: st_meta.widget_type.to_string(),
                        sub_type: st_meta.sub_type.to_string(),
                        x: 0,
                        y: 0,
                        width: pv_w as i32,
                        height: pv_h as i32,
                        settings: None,
                    };
                    render_widget_card(
                        &sw.surface.gfx,
                        &sw.surface.dc,
                        &preview_layout,
                        pv_r,
                        theme,
                        &settings.dimensions,
                        &font,
                        &lang,
                        monitor,
                        weather,
                        &settings.weather,
                        timezones,
                        None,
                        &mut dummy_hits,
                        true,
                        None,
                    );

                    // Right: Title, subtitle, and "+ Add Widget" button
                    let info_x = pv_r.right + 24.0;
                    draw_text(
                        &sw.surface.gfx,
                        &sw.surface.dc,
                        tr(&lang, st_meta.title_key),
                        rect(info_x, cy + 26.0, card_r.right - 20.0, cy + 54.0),
                        &font,
                        18.0,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                        pal.fg,
                        false,
                    );
                    draw_text(
                        &sw.surface.gfx,
                        &sw.surface.dc,
                        tr(&lang, st_meta.subtitle_key),
                        rect(info_x, cy + 58.0, card_r.right - 20.0, cy + 120.0),
                        &font,
                        13.0,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                        pal.muted,
                        true,
                    );
                    let add_r = rect(info_x, cy + 132.0, info_x + 132.0, cy + 168.0);
                    draw_pill_btn(sw, &pal, add_r, "+ Add Widget", &font, true, false);
                    hits.push(HitBox {
                        rect: add_r,
                        action: UiAction::SpawnGallery {
                            widget_type: st_meta.widget_type.to_string(),
                            sub_type: st_meta.sub_type.to_string(),
                            cols: st_meta.default_cols,
                            rows: st_meta.default_rows,
                        },
                    });

                    cy += row_h + 14.0;
                }
            }
        }
    }

    unsafe {
        sw.surface.dc.PopAxisAlignedClip();
    }

    let total_content_h = cy + sw.scroll_y + 20.0;
    sw.max_scroll = (total_content_h - h).max(0.0);
    sw.scroll_y = sw.scroll_y.clamp(0.0, sw.max_scroll);

    // Subtle right-edge scrollbar indicator when page overflows vertically
    if sw.max_scroll > 1.0 && total_content_h > h {
        let track_pad = 8.0_f32;
        let track_h = (h - track_pad * 2.0).max(24.0);
        let thumb_h = (track_h * (h / total_content_h)).clamp(32.0, track_h);
        let scroll_frac = (sw.scroll_y / sw.max_scroll).clamp(0.0, 1.0);
        let thumb_y = track_pad + (track_h - thumb_h) * scroll_frac;
        let sb_col = if pal.is_dark {
            rgba(1.0, 1.0, 1.0, 0.22)
        } else {
            rgba(0.0, 0.0, 0.0, 0.22)
        };
        fill_round_rect(
            &sw.surface.dc,
            rect(w - 9.0, thumb_y, w - 4.0, thumb_y + thumb_h),
            2.5,
            sb_col,
        );
    }

    sw.hits = hits;

    let _ = sw.surface.end();
}

fn update_slider_from_x(
    app: &Rc<RefCell<AppState>>,
    kind: SliderKind,
    slider: D2D_RECT_F,
    dip_x: f32,
) {
    let frac =
        f64::from(((dip_x - slider.left) / (slider.right - slider.left).max(1.0)).clamp(0.0, 1.0));
    let changed = {
        let mut st = app.borrow_mut();
        match kind {
            SliderKind::Opacity => {
                let rounded = ((0.05 + frac * 0.95).clamp(0.05, 1.0) * 100.0).round() / 100.0;
                if (st.settings.theme.opacity_level - rounded).abs() >= 0.005 {
                    st.settings.theme.opacity_level = rounded;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgBlur => {
                let v = (frac * 30.0).round() as i32;
                if st.settings.liquid_glass.blur_radius != v {
                    st.settings.liquid_glass.blur_radius = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgRefraction => {
                let v = ((frac * 2.0) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.refraction - v).abs() >= 0.005 {
                    st.settings.liquid_glass.refraction = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgBevelWidth => {
                let v = ((0.3 + frac * 1.5) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.bevel_width - v).abs() >= 0.005 {
                    st.settings.liquid_glass.bevel_width = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgDispersion => {
                let v = ((frac * 2.5) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.dispersion - v).abs() >= 0.005 {
                    st.settings.liquid_glass.dispersion = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgSpecular => {
                let v = ((frac * 2.0) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.specular - v).abs() >= 0.005 {
                    st.settings.liquid_glass.specular = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgVibrancy => {
                let v = ((0.5 + frac * 1.5) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.vibrancy - v).abs() >= 0.005 {
                    st.settings.liquid_glass.vibrancy = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::LgGrain => {
                let v = ((frac * 2.5) * 20.0).round() / 20.0;
                if (st.settings.liquid_glass.frost_grain - v).abs() >= 0.005 {
                    st.settings.liquid_glass.frost_grain = v;
                    true
                } else {
                    false
                }
            }
            SliderKind::CornerRadius => {
                let v = (frac * 44.0).round() as i32;
                if st.settings.dimensions.radius != v {
                    st.settings.dimensions.radius = v;
                    true
                } else {
                    false
                }
            }
        }
    };
    if changed {
        enqueue_cmd(AppCommand::ApplySettingsBroadcast);
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
                paint_settings(&mut st);
            }
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                if let Some(sw) = st.settings_win.as_mut() {
                    let mut rc = RECT::default();
                    let dpi = unsafe {
                        let _ = GetClientRect(hwnd, &mut rc);
                        (GetDpiForWindow(hwnd) as f32).max(96.0)
                    };
                    let _ = sw.surface.resize(
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
            let delta = ((wparam.0 >> 16) as i16) as f32;
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                if let Some(sw) = st.settings_win.as_mut() {
                    sw.scroll_y = (sw.scroll_y - delta * 0.45).clamp(0.0, sw.max_scroll);
                    sw.invalidate();
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (px, py) = lparam_point(lparam);
            if let Some(app) = get_global_app() {
                let hit_action = {
                    let mut st = app.borrow_mut();
                    let act = st.settings_win.as_mut().and_then(|sw| {
                        let dx = px as f32 * 96.0 / sw.surface.dpi;
                        let dy = py as f32 * 96.0 / sw.surface.dpi;
                        sw.hits
                            .iter()
                            .rev()
                            .find(|h| {
                                dx >= h.rect.left
                                    && dx <= h.rect.right
                                    && dy >= h.rect.top
                                    && dy <= h.rect.bottom
                            })
                            .map(|h| (h.action.clone(), dx))
                    });
                    if let Some((UiAction::DragSlider(kind, sl), _)) = &act
                        && let Some(sw) = st.settings_win.as_mut()
                    {
                        sw.dragging_slider = Some((*kind, *sl));
                    }
                    let clicked_weather_input =
                        matches!(&act, Some((UiAction::FocusWeatherSearch, _)));
                    if !clicked_weather_input
                        && let Some(sw) = st.settings_win.as_mut()
                        && sw.weather_input_focused
                    {
                        sw.weather_input_focused = false;
                        sw.invalidate();
                    }
                    act
                };

                if let Some((action, dx)) = hit_action {
                    match action {
                        UiAction::SelectPage(p) => {
                            let mut st = app.borrow_mut();
                            if let Some(sw) = st.settings_win.as_mut() {
                                sw.page = p;
                                sw.scroll_y = 0.0;
                                sw.invalidate();
                            }
                        }
                        UiAction::TogglePane => {
                            let mut st = app.borrow_mut();
                            if let Some(sw) = st.settings_win.as_mut() {
                                let w = sw.surface.width as f32 * 96.0 / sw.surface.dpi;
                                let cur = sw.pane_override.unwrap_or(w >= 800.0);
                                sw.pane_override = Some(!cur);
                                sw.invalidate();
                            }
                        }
                        UiAction::SelectLanguage(code) => {
                            {
                                let mut st = app.borrow_mut();
                                if st.settings.region.language != code {
                                    st.settings.region.language = code;
                                    st.language_changed = true;
                                    st.save_settings();
                                }
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleRunOnStartup => {
                            let enabled = {
                                let mut st = app.borrow_mut();
                                st.settings.run_on_startup = !st.settings.run_on_startup;
                                st.settings.run_on_startup
                            };
                            let _ = crate::platform::system::set_run_on_startup(enabled);
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::RestartApp => enqueue_cmd(AppCommand::RestartApp),
                        UiAction::QuitApp => enqueue_cmd(AppCommand::QuitApp),
                        UiAction::ApplyTemplate(idx) => {
                            {
                                let mut st = app.borrow_mut();
                                if let Some(tpl) = st.settings.templates.get(idx).cloned() {
                                    st.settings.theme = tpl;
                                }
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ApplyApplePreset => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.font_family = "Inter".to_string();
                                st.settings.theme.use_native_frame = false;
                                st.settings.theme.opacity_level = 0.28;
                                st.settings.theme.monochrome = false;
                                st.settings.dimensions.radius = 26;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ApplyWindowsPreset => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.font_family = "Segoe UI Variable".to_string();
                                st.settings.theme.use_native_frame = true;
                                st.settings.theme.opacity_level = 0.45;
                                st.settings.dimensions.radius = 8;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetDarkMode(dm) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.dark_mode = dm;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetAccentColor(col) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.accent_color = col;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::DragSlider(kind, sl) => {
                            unsafe { SetCapture(hwnd) };
                            update_slider_from_x(&app, kind, sl, dx);
                        }
                        UiAction::ToggleMonochrome => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.monochrome = !st.settings.theme.monochrome;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::EnableLiquidGlassMode => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.use_native_frame = false;
                                if st.settings.theme.opacity_level >= 0.95 {
                                    st.settings.theme.opacity_level = 0.28;
                                }
                                if st.settings.dimensions.radius < 18 {
                                    st.settings.dimensions.radius = 24;
                                }
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ApplyLgPreset(idx) => {
                            {
                                let mut st = app.borrow_mut();
                                let (blur, refr, bev, disp, spec, vib, grain) =
                                    lg_preset_values(idx);
                                st.settings.liquid_glass.blur_radius = blur;
                                st.settings.liquid_glass.refraction = refr;
                                st.settings.liquid_glass.bevel_width = bev;
                                st.settings.liquid_glass.dispersion = disp;
                                st.settings.liquid_glass.specular = spec;
                                st.settings.liquid_glass.vibrancy = vib;
                                st.settings.liquid_glass.frost_grain = grain;
                                // Automatically activate Liquid Glass framing if currently in native Win11 frame or 100% solid
                                if st.settings.theme.use_native_frame {
                                    st.settings.theme.use_native_frame = false;
                                    if st.settings.dimensions.radius < 18 {
                                        st.settings.dimensions.radius = 24;
                                    }
                                }
                                if st.settings.theme.opacity_level >= 0.95 {
                                    st.settings.theme.opacity_level = 0.28;
                                }
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ResetLiquidGlass => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.liquid_glass = LiquidGlassConfig::default();
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetLgTint(col) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.liquid_glass.tint_color = col;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::FocusWeatherSearch => {
                            let mut st = app.borrow_mut();
                            if let Some(sw) = st.settings_win.as_mut() {
                                sw.weather_input_focused = true;
                                sw.invalidate();
                            }
                        }
                        UiAction::TriggerWeatherSearch => {
                            let q = {
                                let mut st = app.borrow_mut();
                                st.settings_win
                                    .as_mut()
                                    .map(|sw| {
                                        sw.weather_status = Some("Searching cities…".to_string());
                                        sw.invalidate();
                                        sw.weather_query.clone()
                                    })
                                    .unwrap_or_default()
                            };
                            enqueue_cmd(AppCommand::SearchWeatherCities(q));
                        }
                        UiAction::PickWeatherCity(idx) => {
                            {
                                let mut st = app.borrow_mut();
                                let city = st
                                    .settings_win
                                    .as_ref()
                                    .and_then(|sw| sw.weather_results.get(idx).cloned());
                                if let Some(city) = city {
                                    st.settings.weather.default_city = city.name.clone();
                                    st.settings.weather.latitude = city.latitude;
                                    st.settings.weather.longitude = city.longitude;
                                    st.apply_weather_to_widgets(true, false);
                                    st.trigger_weather_refresh(true);
                                    if let Some(sw) = st.settings_win.as_mut() {
                                        sw.weather_query = city.name;
                                        sw.weather_results.clear();
                                        sw.weather_status = Some(
                                            "Location updated — fetching forecast…".to_string(),
                                        );
                                    }
                                }
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::DetectWeatherLocation => {
                            {
                                let mut st = app.borrow_mut();
                                if let Some(sw) = st.settings_win.as_mut() {
                                    sw.weather_status =
                                        Some("Detecting current location…".to_string());
                                    sw.invalidate();
                                }
                            }
                            enqueue_cmd(AppCommand::DetectWeatherLocation);
                        }
                        UiAction::RefreshWeatherNow => {
                            {
                                let mut st = app.borrow_mut();
                                if let Some(sw) = st.settings_win.as_mut() {
                                    sw.weather_status = Some("Refreshing forecast…".to_string());
                                    sw.invalidate();
                                }
                            }
                            enqueue_cmd(AppCommand::RefreshWeatherNow);
                        }
                        UiAction::SetWeatherTempUnit(unit) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.temperature_unit = unit;
                                st.apply_weather_to_widgets(false, true);
                                st.trigger_weather_refresh(true);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetWeatherWindUnit(unit) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.wind_unit = unit;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetWeatherRefreshMin(mins) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.refresh_minutes = mins.clamp(5, 180);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetWeatherHourlyCount(slots) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.hourly_count = slots.clamp(4, 8);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleWeatherHighLow => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.show_high_low =
                                    !st.settings.weather.show_high_low;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleWeatherCondition => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.show_condition =
                                    !st.settings.weather.show_condition;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleWeatherFeelsLike => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.show_feels_like =
                                    !st.settings.weather.show_feels_like;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleWeatherHumidityWind => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.show_humidity_wind =
                                    !st.settings.weather.show_humidity_wind;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleWeather24H => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.weather.use_24_hour_clock =
                                    !st.settings.weather.use_24_hour_clock;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ResetWeatherDefaults => {
                            {
                                let mut st = app.borrow_mut();
                                let keep_city = st.settings.weather.default_city.clone();
                                let keep_lat = st.settings.weather.latitude;
                                let keep_lon = st.settings.weather.longitude;
                                st.settings.weather = WeatherConfig {
                                    default_city: keep_city,
                                    latitude: keep_lat,
                                    longitude: keep_lon,
                                    ..WeatherConfig::default()
                                };
                                st.apply_weather_to_widgets(false, true);
                                st.trigger_weather_refresh(false);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleNativeFrame => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.use_native_frame =
                                    !st.settings.theme.use_native_frame;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SetFontFamily(fam) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.theme.font_family = fam;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::AdjustSize(delta) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.dimensions.size =
                                    (st.settings.dimensions.size + delta).clamp(36, 144);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::AdjustMargin(delta) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.dimensions.margin =
                                    (st.settings.dimensions.margin + delta).clamp(0, 32);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::AdjustRadius(delta) => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.dimensions.radius =
                                    (st.settings.dimensions.radius + delta).clamp(0, 48);
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleSnapPosition => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.layout.snap_position =
                                    !st.settings.layout.snap_position;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleLockPosition => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.layout.lock_position =
                                    !st.settings.layout.lock_position;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleSnapSize => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.layout.snap_size = !st.settings.layout.snap_size;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::ToggleLockSize => {
                            {
                                let mut st = app.borrow_mut();
                                st.settings.layout.lock_size = !st.settings.layout.lock_size;
                            }
                            enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                        }
                        UiAction::SpawnGallery {
                            widget_type,
                            sub_type,
                            cols,
                            rows,
                        } => {
                            enqueue_cmd(AppCommand::SpawnWidget {
                                widget_type,
                                sub_type,
                                cols,
                                rows,
                            });
                        }
                        UiAction::OpenUrl(url) => open_url(url),
                        UiAction::CheckUpdates => enqueue_cmd(AppCommand::CheckForUpdatesManual),
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (px, _py) = lparam_point(lparam);
            if let Some(app) = get_global_app() {
                let drag = {
                    let st = app.borrow();
                    st.settings_win.as_ref().and_then(|sw| {
                        sw.dragging_slider
                            .map(|(kind, sl)| (kind, sl, px as f32 * 96.0 / sw.surface.dpi))
                    })
                };
                if let Some((kind, sl, dx)) = drag {
                    update_slider_from_x(&app, kind, sl, dx);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if let Some(app) = get_global_app() {
                let was_dragging = {
                    let mut st = app.borrow_mut();
                    st.settings_win
                        .as_mut()
                        .and_then(|sw| sw.dragging_slider.take())
                        .is_some()
                };
                if was_dragging {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            if let Some(app) = get_global_app() {
                let search_focused = {
                    let st = app.borrow();
                    st.settings_win
                        .as_ref()
                        .is_some_and(|sw| sw.page == NavPage::Weather && sw.weather_input_focused)
                };
                if search_focused {
                    let ctrl =
                        unsafe { (GetKeyState(i32::from(VK_CONTROL.0)) as u16 & 0x8000) != 0 };
                    if vk == VK_ESCAPE.0 {
                        let mut st = app.borrow_mut();
                        if let Some(sw) = st.settings_win.as_mut() {
                            sw.weather_input_focused = false;
                            sw.invalidate();
                        }
                        return LRESULT(0);
                    }
                    if vk == VK_BACK.0 {
                        let mut st = app.borrow_mut();
                        if let Some(sw) = st.settings_win.as_mut() {
                            sw.weather_query.pop();
                            sw.invalidate();
                        }
                        return LRESULT(0);
                    }
                    if vk == VK_RETURN.0 {
                        let q = {
                            let mut st = app.borrow_mut();
                            st.settings_win
                                .as_mut()
                                .map(|sw| {
                                    sw.weather_status = Some("Searching cities…".to_string());
                                    sw.invalidate();
                                    sw.weather_query.clone()
                                })
                                .unwrap_or_default()
                        };
                        enqueue_cmd(AppCommand::SearchWeatherCities(q));
                        return LRESULT(0);
                    }
                    if ctrl && vk == b'V' as u16 {
                        if let Some(txt) = get_clipboard_text(hwnd) {
                            let cleaned = txt.replace(['\r', '\n'], " ");
                            let mut st = app.borrow_mut();
                            if let Some(sw) = st.settings_win.as_mut() {
                                sw.weather_query.push_str(&cleaned);
                                sw.invalidate();
                            }
                        }
                        return LRESULT(0);
                    }
                    return LRESULT(0);
                }

                // Original uWidgets Easter egg: typing "uwu" changes header title to "UwUidgets"
                let ch = (wparam.0 as u8 as char).to_ascii_lowercase();
                if ch.is_ascii_alphabetic() {
                    let mut st = app.borrow_mut();
                    if let Some(sw) = st.settings_win.as_mut() {
                        sw.easter_buf.push(ch);
                        if sw.easter_buf.len() > 8 {
                            let start = sw.easter_buf.len() - 8;
                            sw.easter_buf = sw.easter_buf[start..].to_string();
                        }
                        if sw.easter_buf.ends_with("uwu") {
                            sw.uwu_easter_egg = !sw.uwu_easter_egg;
                            sw.easter_buf.clear();
                            sw.invalidate();
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam.0 as u32)
                && !ch.is_control()
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                if let Some(sw) = st.settings_win.as_mut()
                    && sw.page == NavPage::Weather
                    && sw.weather_input_focused
                {
                    sw.weather_query.push(ch);
                    sw.invalidate();
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            enqueue_cmd(AppCommand::CloseSettings);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// Keep unused Theme import warning-free
#[allow(dead_code)]
fn _theme_check(_: &Theme) {}
