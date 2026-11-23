pub mod edit_window;
pub mod settings_window;
pub mod tray;
pub mod update_popup;
pub mod widget_window;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, PostMessageW, PostQuitMessage, SetTimer, WM_APP,
};

use crate::core_rs;
use crate::models::{
    AppSettings, ClockModel, ForecastModel, ResolvedTheme, WidgetLayout, load_app_settings,
    load_layout, resolve_theme, save_app_settings, save_layout,
};
use crate::platform::graphics::SharedGraphics;
use crate::platform::system::{
    MonitorCollector, WinTimeZone, get_windows_accent_color, get_windows_dark_mode,
    load_system_timezones,
};
use crate::updater::{
    ApplyResult, InstallChannel, UpdateCheckOutcome, UpdateInfo, apply_update_sync,
    check_for_updates, detect_install_channel,
};
use crate::weather_client::{
    GeoCity, WeatherService, detect_current_location_sync, search_cities_sync,
};

pub const APP_VERSION: &str = "1.0.0";

pub const WM_APP_TRAY: u32 = WM_APP + 1;
pub const WM_APP_DISPATCH: u32 = WM_APP + 2;
pub const WM_APP_WEATHER_READY: u32 = WM_APP + 3;
pub const WM_APP_CITY_RESULTS: u32 = WM_APP + 4;
pub const WM_APP_UPDATE_RESULT: u32 = WM_APP + 5;
pub const WM_APP_DETECT_LOCATION: u32 = WM_APP + 6;
pub const WM_APP_UPDATE_APPLIED: u32 = WM_APP + 7;

pub const TIMER_100MS: usize = 101;
pub const TIMER_1S: usize = 102;
pub const TIMER_5S: usize = 103;
pub const TIMER_1M: usize = 104;
pub const TIMER_5M: usize = 105;
pub const TIMER_1H: usize = 106;

#[derive(Debug, Clone)]
pub enum AppCommand {
    OpenSettings,
    CloseSettings,
    OpenEditWidget(u64),
    CloseEditWidget,
    OpenUpdatePopup(UpdateInfo),
    CloseUpdatePopup,
    SpawnWidget {
        widget_type: String,
        sub_type: String,
        cols: i64,
        rows: i64,
    },
    RemoveWidget(u64),
    ResizeWidgetPreset {
        id: u64,
        preset_index: i64,
    },
    ApplySettingsBroadcast,
    ReloadAllWidgets,
    CheckForUpdatesManual,
    CheckForUpdatesAuto,
    InstallPendingUpdate(UpdateInfo),
    SearchWeatherCities(String),
    DetectWeatherLocation,
    RefreshWeatherNow,
    RestartApp,
    QuitApp,
}

pub struct AppState {
    pub config_dir: PathBuf,
    pub settings: AppSettings,
    pub layouts: Vec<(u64, WidgetLayout)>,
    pub system_dark: bool,
    pub system_accent: (f32, f32, f32),
    pub gfx: Rc<SharedGraphics>,
    pub monitor: MonitorCollector,
    pub weather: WeatherService,
    pub timezones: Vec<WinTimeZone>,
    pub msg_hwnd: HWND,
    pub tray: Option<tray::TrayIcon>,
    pub widget_windows: HashMap<u64, widget_window::WidgetWindow>,
    pub settings_win: Option<settings_window::SettingsWindow>,
    pub edit_win: Option<edit_window::EditWindow>,
    pub update_win: Option<update_popup::UpdatePopup>,
    pub next_id: u64,
    pub language_changed: bool,
    pub update_status: Option<String>,
    pub pending_update: Option<UpdateInfo>,
    pub install_channel: InstallChannel,
    pub update_hours_elapsed: u32,
    timer_100ms_active: bool,
    timer_1s_active: bool,
}

thread_local! {
    static GLOBAL_APP: RefCell<Option<Rc<RefCell<AppState>>>> = const { RefCell::new(None) };
    static CMD_QUEUE: RefCell<VecDeque<AppCommand>> = const { RefCell::new(VecDeque::new()) };
    static HOST_HWND: Cell<isize> = const { Cell::new(0) };
}

pub fn set_global_app(app: Rc<RefCell<AppState>>) {
    let hwnd_raw = app.borrow().msg_hwnd.0 as isize;
    HOST_HWND.with(|h| h.set(hwnd_raw));
    GLOBAL_APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}

pub fn get_global_app() -> Option<Rc<RefCell<AppState>>> {
    GLOBAL_APP.with(|cell| cell.borrow().clone())
}

pub fn clear_global_app() {
    HOST_HWND.with(|h| h.set(0));
    CMD_QUEUE.with(|q| q.borrow_mut().clear());
    GLOBAL_APP.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub fn enqueue_cmd(cmd: AppCommand) {
    // Decoupled from `AppState`'s RefCell so enqueuing commands can never panic on re-entrancy.
    CMD_QUEUE.with(|q| q.borrow_mut().push_back(cmd));
    let hwnd_raw = HOST_HWND.with(Cell::get);
    if hwnd_raw != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(hwnd_raw as *mut _),
                WM_APP_DISPATCH,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

impl AppState {
    pub fn new(config_dir: PathBuf, gfx: Rc<SharedGraphics>, msg_hwnd: HWND) -> Self {
        let mut settings = load_app_settings(&config_dir);
        let raw_layouts = load_layout(&config_dir);
        let mut next_id = 1u64;
        let layouts: Vec<(u64, WidgetLayout)> = raw_layouts
            .into_iter()
            .map(|w| {
                let id = next_id;
                next_id += 1;
                (id, w)
            })
            .collect();

        // If Weather settings were defaulted, seed default city/unit from existing desktop Weather widget.
        if settings.weather.default_city == "Cupertino"
            && (settings.weather.latitude - 37.323).abs() < 0.01
            && let Some((_, w)) = layouts.iter().find(|(_, l)| l.widget_type == "Weather")
        {
            let m: ForecastModel = w.parse_settings();
            if !m.name.trim().is_empty() {
                settings.weather.default_city = m.name;
                settings.weather.latitude = m.latitude;
                settings.weather.longitude = m.longitude;
                if !m.temperature_unit.is_empty() {
                    settings.weather.temperature_unit = m.temperature_unit;
                }
            }
        }

        let weather = WeatherService::new(config_dir.clone(), msg_hwnd);
        let mut monitor = MonitorCollector::new();
        let has_monitor = layouts.iter().any(|(_, w)| w.widget_type == "Monitor");
        if has_monitor {
            monitor.sample();
        }
        let needs_tz = layouts.iter().any(|(_, w)| {
            w.widget_type == "Clock"
                && (w.sub_type == "World"
                    || w.parse_settings::<ClockModel>().time_zone_id.is_some())
        });
        let timezones = if needs_tz {
            load_system_timezones()
        } else {
            Vec::new()
        };

        Self {
            config_dir,
            settings,
            layouts,
            system_dark: get_windows_dark_mode(),
            system_accent: get_windows_accent_color(),
            gfx,
            monitor,
            weather,
            timezones,
            msg_hwnd,
            tray: None,
            widget_windows: HashMap::new(),
            settings_win: None,
            edit_win: None,
            update_win: None,
            next_id,
            language_changed: false,
            update_status: None,
            pending_update: None,
            install_channel: detect_install_channel(),
            update_hours_elapsed: 0,
            timer_100ms_active: false,
            timer_1s_active: false,
        }
    }

    pub fn ensure_timezones(&mut self) {
        if self.timezones.is_empty() {
            self.timezones = load_system_timezones();
        }
    }

    pub fn has_monitor_widgets(&self) -> bool {
        self.layouts.iter().any(|(_, w)| w.widget_type == "Monitor")
    }

    pub fn resolved_theme(&self) -> ResolvedTheme {
        resolve_theme(
            &self.settings.theme,
            &self.settings.dimensions,
            self.system_dark,
            self.system_accent,
        )
    }

    pub fn save_settings(&self) {
        let _ = save_app_settings(&self.config_dir, &self.settings);
    }

    pub fn save_layouts(&self) {
        let vec: Vec<WidgetLayout> = self.layouts.iter().map(|(_, w)| w.clone()).collect();
        let _ = save_layout(&self.config_dir, &vec);
    }

    pub fn find_layout(&self, id: u64) -> Option<&WidgetLayout> {
        self.layouts.iter().find(|(i, _)| *i == id).map(|(_, w)| w)
    }

    pub fn find_layout_mut(&mut self, id: u64) -> Option<&mut WidgetLayout> {
        self.layouts
            .iter_mut()
            .find(|(i, _)| *i == id)
            .map(|(_, w)| w)
    }

    pub fn start_base_timers(&mut self) {
        unsafe {
            let _ = SetTimer(self.msg_hwnd, TIMER_5S, 5_000, None);
            let _ = SetTimer(self.msg_hwnd, TIMER_1M, 60_000, None);
            let _ = SetTimer(self.msg_hwnd, TIMER_5M, 300_000, None);
            let _ = SetTimer(self.msg_hwnd, TIMER_1H, 3_600_000, None);
        }
        self.sync_adaptive_timers();
    }

    /// Adaptive timer scheduler: only enables 100ms or 1s ticks when an active widget/view needs them!
    pub fn sync_adaptive_timers(&mut self) {
        let need_100ms = self.layouts.iter().any(|(_, w)| {
            w.widget_type == "Clock"
                && matches!(w.sub_type.as_str(), "AnalogI" | "AnalogII" | "AnalogIII")
                && w.parse_settings::<ClockModel>().show_seconds
        });
        let need_1s = self.settings_win.is_some()
            || self.layouts.iter().any(|(_, w)| {
                w.widget_type == "Monitor"
                    || (w.widget_type == "Clock"
                        && matches!(w.sub_type.as_str(), "Digital" | "Flip")
                        && w.parse_settings::<ClockModel>().show_seconds)
            });

        unsafe {
            if need_100ms && !self.timer_100ms_active {
                let _ = SetTimer(self.msg_hwnd, TIMER_100MS, 100, None);
                self.timer_100ms_active = true;
            } else if !need_100ms && self.timer_100ms_active {
                let _ = KillTimer(self.msg_hwnd, TIMER_100MS);
                self.timer_100ms_active = false;
            }

            if need_1s && !self.timer_1s_active {
                let _ = SetTimer(self.msg_hwnd, TIMER_1S, 1_000, None);
                self.timer_1s_active = true;
            } else if !need_1s && self.timer_1s_active {
                let _ = KillTimer(self.msg_hwnd, TIMER_1S);
                self.timer_1s_active = false;
            }
        }
    }

    pub fn invalidate_widgets_matching<F: Fn(&WidgetLayout) -> bool>(&self, pred: F) {
        for (id, layout) in &self.layouts {
            if pred(layout)
                && let Some(win) = self.widget_windows.get(id)
            {
                win.invalidate();
            }
        }
    }

    pub fn invalidate_all_widgets(&self) {
        for win in self.widget_windows.values() {
            win.invalidate();
        }
    }

    pub fn trigger_weather_refresh(&self, force: bool) {
        let lang = self.settings.region.language.clone();
        let ttl_secs = (self.settings.weather.refresh_minutes.clamp(5, 180) as u64) * 60;
        let mut fetched_any = false;
        for (_, w) in &self.layouts {
            if w.widget_type == "Weather" {
                let m: ForecastModel = w.parse_settings();
                let _ = self.weather.get_or_fetch(&m, &lang, ttl_secs, force);
                fetched_any = true;
            }
        }
        if !fetched_any || force {
            let def_m = self.settings.weather.to_forecast_model();
            let _ = self.weather.get_or_fetch(&def_m, &lang, ttl_secs, force);
        }
    }

    /// Syncs the default Weather city/coordinates/unit across all active Weather widgets on desktop.
    pub fn apply_weather_to_widgets(&mut self, update_city: bool, update_unit: bool) {
        let city = self.settings.weather.default_city.clone();
        let lat = self.settings.weather.latitude;
        let lon = self.settings.weather.longitude;
        let unit = self.settings.weather.temperature_unit.clone();
        let mut changed_layout = false;
        for (_, l) in &mut self.layouts {
            if l.widget_type == "Weather" {
                let mut m: ForecastModel = l.parse_settings();
                if update_city {
                    m.name = city.clone();
                    m.latitude = lat;
                    m.longitude = lon;
                }
                if update_unit {
                    m.temperature_unit = unit.clone();
                }
                l.set_settings(&m);
                changed_layout = true;
            }
        }
        if changed_layout {
            self.save_layouts();
        }
    }

    pub fn refresh_system_theme(&mut self) {
        self.system_dark = get_windows_dark_mode();
        self.system_accent = get_windows_accent_color();
    }
}

pub fn drain_commands(app: &Rc<RefCell<AppState>>) {
    loop {
        let cmd = CMD_QUEUE.with(|q| q.borrow_mut().pop_front());
        let Some(cmd) = cmd else { break };
        handle_command(app, cmd);
    }
}

fn handle_command(app: &Rc<RefCell<AppState>>, cmd: AppCommand) {
    match cmd {
        AppCommand::OpenSettings => {
            settings_window::open_or_focus(app);
            app.borrow_mut().sync_adaptive_timers();
        }
        AppCommand::CloseSettings => {
            let hwnd = {
                let mut st = app.borrow_mut();
                st.settings_win.take().map(|w| w.hwnd)
            };
            if let Some(h) = hwnd {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(h);
                }
            }
            app.borrow_mut().sync_adaptive_timers();
        }
        AppCommand::OpenEditWidget(id) => {
            app.borrow_mut().ensure_timezones();
            edit_window::open_for_widget(app, id);
        }
        AppCommand::CloseEditWidget => {
            let hwnd = {
                let mut st = app.borrow_mut();
                st.edit_win.take().map(|w| w.hwnd)
            };
            if let Some(h) = hwnd {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(h);
                }
            }
        }
        AppCommand::OpenUpdatePopup(info) => {
            update_popup::open_popup(app, info);
        }
        AppCommand::CloseUpdatePopup => {
            let hwnd = {
                let mut st = app.borrow_mut();
                st.update_win.take().map(|w| w.hwnd)
            };
            if let Some(h) = hwnd {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(h);
                }
            }
        }
        AppCommand::SpawnWidget {
            widget_type,
            sub_type,
            cols,
            rows,
        } => {
            let (id, is_weather) = {
                let mut st = app.borrow_mut();
                let dims = st.settings.dimensions.clone();
                let w = core_rs::port::grid_get_size(
                    cols,
                    i64::from(dims.size),
                    i64::from(dims.margin),
                    1.0,
                    false,
                )
                .unwrap_or(160) as i32;
                let h = core_rs::port::grid_get_size(
                    rows,
                    i64::from(dims.size),
                    i64::from(dims.margin),
                    1.0,
                    false,
                )
                .unwrap_or(160) as i32;
                let offset = (st.layouts.len() as i32 % 8) * 28;
                let id = st.next_id;
                st.next_id += 1;
                let is_w = widget_type == "Weather";
                let init_settings = if is_w {
                    serde_json::to_value(st.settings.weather.to_forecast_model()).ok()
                } else {
                    None
                };
                let layout = WidgetLayout {
                    widget_type,
                    sub_type,
                    x: 120 + offset,
                    y: 120 + offset,
                    width: w,
                    height: h,
                    settings: init_settings,
                };
                st.layouts.push((id, layout));
                st.save_layouts();
                (id, is_w)
            };
            widget_window::spawn_widget_window(app, id);
            let mut st = app.borrow_mut();
            if st.has_monitor_widgets() {
                st.monitor.sample();
            }
            st.sync_adaptive_timers();
            if is_weather {
                st.trigger_weather_refresh(false);
            }
        }
        AppCommand::RemoveWidget(id) => {
            let (hwnd, close_edit) = {
                let mut st = app.borrow_mut();
                st.layouts.retain(|(i, _)| *i != id);
                st.save_layouts();
                let w = st.widget_windows.remove(&id).map(|ww| ww.hwnd);
                let ce = st.edit_win.as_ref().is_some_and(|e| e.widget_id == id);
                (w, ce)
            };
            if let Some(h) = hwnd {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(h);
                }
            }
            if close_edit {
                enqueue_cmd(AppCommand::CloseEditWidget);
            }
            app.borrow_mut().sync_adaptive_timers();
        }
        AppCommand::ResizeWidgetPreset { id, preset_index } => {
            let resize_info = {
                let mut st = app.borrow_mut();
                let dims = st.settings.dimensions.clone();
                let cols = core_rs::port::preset_columns(preset_index).unwrap_or(2);
                let rows = core_rs::port::preset_rows(preset_index).unwrap_or(2);
                let nw = core_rs::port::grid_get_size(
                    cols,
                    i64::from(dims.size),
                    i64::from(dims.margin),
                    1.0,
                    false,
                )
                .unwrap_or(160) as i32;
                let nh = core_rs::port::grid_get_size(
                    rows,
                    i64::from(dims.size),
                    i64::from(dims.margin),
                    1.0,
                    false,
                )
                .unwrap_or(160) as i32;
                if let Some(layout) = st.find_layout_mut(id) {
                    layout.width = nw;
                    layout.height = nh;
                }
                st.save_layouts();
                st.widget_windows.get(&id).map(|w| (w.hwnd, nw, nh))
            };
            if let Some((hwnd, nw, nh)) = resize_info {
                widget_window::resize_widget_hwnd(hwnd, nw, nh);
            }
        }
        AppCommand::ApplySettingsBroadcast => {
            let (theme, widget_hwnds, settings_hwnd, edit_hwnd) = {
                let st = app.borrow();
                st.save_settings();
                (
                    st.resolved_theme(),
                    st.widget_windows
                        .values()
                        .map(|w| w.hwnd)
                        .collect::<Vec<_>>(),
                    st.settings_win.as_ref().map(|w| w.hwnd),
                    st.edit_win.as_ref().map(|w| w.hwnd),
                )
            };
            for h in widget_hwnds {
                crate::platform::system::apply_window_dwm(
                    h,
                    theme.is_dark,
                    theme.uses_acrylic,
                    theme.use_native_frame,
                );
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(h, None, false);
                }
            }
            if let Some(h) = settings_hwnd {
                settings_window::apply_titlebar_theme(h, theme.is_dark);
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(h, None, false);
                }
            }
            if let Some(h) = edit_hwnd {
                settings_window::apply_titlebar_theme(h, theme.is_dark);
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::InvalidateRect(h, None, false);
                }
            }
        }
        AppCommand::ReloadAllWidgets => {
            let hwnds: Vec<HWND> = {
                let mut st = app.borrow_mut();
                st.widget_windows.drain().map(|(_, w)| w.hwnd).collect()
            };
            for h in hwnds {
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(h);
                }
            }
            let ids: Vec<u64> = {
                let mut st = app.borrow_mut();
                st.settings = load_app_settings(&st.config_dir);
                let raw = load_layout(&st.config_dir);
                st.layouts.clear();
                for w in raw {
                    let id = st.next_id;
                    st.next_id += 1;
                    st.layouts.push((id, w));
                }
                st.layouts.iter().map(|(id, _)| *id).collect()
            };
            for id in ids {
                widget_window::spawn_widget_window(app, id);
            }
            let mut st = app.borrow_mut();
            st.sync_adaptive_timers();
            st.trigger_weather_refresh(true);
        }
        AppCommand::CheckForUpdatesManual | AppCommand::CheckForUpdatesAuto => {
            let manual = matches!(cmd, AppCommand::CheckForUpdatesManual);
            let (cfg, ignore, hwnd_raw) = {
                let mut st = app.borrow_mut();
                if manual {
                    st.update_status = Some(match st.install_channel {
                        InstallChannel::MicrosoftStore => {
                            "Checking Microsoft Store for updates…".to_string()
                        }
                        _ => "Checking GitHub Releases & NeosparkX…".to_string(),
                    });
                    if let Some(sw) = &st.settings_win {
                        sw.invalidate();
                    }
                }
                (
                    st.settings.updates.clone(),
                    st.settings.ignore_update.clone(),
                    st.msg_hwnd.0 as isize,
                )
            };
            let allow_auto = !manual && cfg.auto_install;
            std::thread::spawn(move || {
                let outcome =
                    check_for_updates(APP_VERSION, &cfg, ignore.as_deref(), allow_auto, manual);
                if let Ok(mut guard) = UPDATE_RESULT_SLOT.lock() {
                    *guard = Some((outcome, manual));
                }
                unsafe {
                    let _ = PostMessageW(
                        HWND(hwnd_raw as *mut _),
                        WM_APP_UPDATE_RESULT,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            });
        }
        AppCommand::InstallPendingUpdate(info) => {
            let (channel, hwnd_raw) = {
                let mut st = app.borrow_mut();
                st.update_status = Some(format!("Downloading & installing v{}…", info.version));
                if let Some(sw) = &st.settings_win {
                    sw.invalidate();
                }
                (st.install_channel, st.msg_hwnd.0 as isize)
            };
            std::thread::spawn(move || {
                let res = apply_update_sync(&info, channel);
                if let Ok(mut guard) = UPDATE_APPLY_SLOT.lock() {
                    *guard = Some(res);
                }
                unsafe {
                    let _ = PostMessageW(
                        HWND(hwnd_raw as *mut _),
                        WM_APP_UPDATE_APPLIED,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            });
        }
        AppCommand::SearchWeatherCities(query) => {
            let (lang2, hwnd_raw) = {
                let st = app.borrow();
                (
                    crate::i18n::two_letter_lang(&st.settings.region.language).to_string(),
                    st.msg_hwnd.0 as isize,
                )
            };
            std::thread::spawn(move || {
                let res = search_cities_sync(&query, &lang2);
                if let Ok(mut guard) = CITY_SEARCH_SLOT.lock() {
                    *guard = Some(res);
                }
                unsafe {
                    let _ = PostMessageW(
                        HWND(hwnd_raw as *mut _),
                        WM_APP_CITY_RESULTS,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            });
        }
        AppCommand::DetectWeatherLocation => {
            let hwnd_raw = app.borrow().msg_hwnd.0 as isize;
            std::thread::spawn(move || {
                let res = detect_current_location_sync();
                if let Ok(mut guard) = DETECT_LOC_SLOT.lock() {
                    *guard = Some(res);
                }
                unsafe {
                    let _ = PostMessageW(
                        HWND(hwnd_raw as *mut _),
                        WM_APP_DETECT_LOCATION,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            });
        }
        AppCommand::RefreshWeatherNow => {
            let st = app.borrow();
            st.trigger_weather_refresh(true);
            st.invalidate_widgets_matching(|w| w.widget_type == "Weather");
            if let Some(sw) = st.settings_win.as_ref() {
                sw.invalidate();
            }
        }
        AppCommand::RestartApp => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = Command::new(exe).arg("--restarted").spawn();
            }
            unsafe { PostQuitMessage(0) };
        }
        AppCommand::QuitApp => {
            unsafe { PostQuitMessage(0) };
        }
    }
}

pub static CITY_SEARCH_SLOT: std::sync::Mutex<Option<Result<Vec<GeoCity>, String>>> =
    std::sync::Mutex::new(None);
pub static DETECT_LOC_SLOT: std::sync::Mutex<Option<Result<GeoCity, String>>> =
    std::sync::Mutex::new(None);
pub static UPDATE_RESULT_SLOT: std::sync::Mutex<Option<(UpdateCheckOutcome, bool)>> =
    std::sync::Mutex::new(None);
pub static UPDATE_APPLY_SLOT: std::sync::Mutex<Option<Result<ApplyResult, String>>> =
    std::sync::Mutex::new(None);
