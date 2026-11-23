#![windows_subsystem = "windows"]

// Direct single-source-of-truth bindings to the 6 `rust-anything` parity-gated port modules
// (avoids linking 6 duplicate `#[no_mangle] rust_anything_call` C-ABI shims into one `.exe`).
#[path = "../../core-rs/src"]
pub mod core_rs {
    #[allow(dead_code)]
    pub mod port;
}
#[path = "../../calendar-rs/src"]
pub mod calendar_rs {
    #[allow(dead_code)]
    pub mod port;
}
#[path = "../../clock-rs/src"]
pub mod clock_rs {
    #[allow(dead_code)]
    pub mod port;
}
#[path = "../../weather-rs/src"]
pub mod weather_rs {
    #[allow(dead_code)]
    pub mod port;
}
#[path = "../../monitor-rs/src"]
pub mod monitor_rs {
    #[allow(dead_code)]
    pub mod port;
}
#[path = "../../notes-reminders-rs/src"]
pub mod notes_reminders_rs {
    #[allow(dead_code)]
    pub mod port;
}

mod i18n;
mod icons;
mod models;
mod platform;
mod ui;
mod updater;
mod weather_client;
mod widgets;

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowW, GetMessageW, HWND_MESSAGE, MSG,
    PostMessageW, RegisterClassW, RegisterWindowMessageW, TranslateMessage, WM_SETTINGCHANGE,
    WM_TIMER, WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_SESSION_UNLOCK,
};
use windows_core::PCWSTR;

use crate::models::config_dir;
use crate::platform::graphics::SharedGraphics;
use crate::platform::system::to_wide;
use crate::ui::{
    AppCommand, AppState, CITY_SEARCH_SLOT, DETECT_LOC_SLOT, TIMER_1H, TIMER_1M, TIMER_1S,
    TIMER_5M, TIMER_5S, TIMER_100MS, UPDATE_APPLY_SLOT, UPDATE_RESULT_SLOT, WM_APP_CITY_RESULTS,
    WM_APP_DETECT_LOCATION, WM_APP_DISPATCH, WM_APP_TRAY, WM_APP_UPDATE_APPLIED,
    WM_APP_UPDATE_RESULT, WM_APP_WEATHER_READY, clear_global_app, drain_commands, enqueue_cmd,
    get_global_app, set_global_app,
};
use crate::updater::{ApplyResult, UpdateCheckOutcome};

const HOST_CLASS: &str = "uWidgetsRsHost";
const HOST_TITLE: &str = "uWidgetsRsHostWindow";
const TIMER_STARTUP_UPDATE: usize = 107;

static mut IPC_OPEN_SETTINGS_MSG: u32 = 0;

fn main() {
    updater::cleanup_old_portable_binary();
    let dir = config_dir();
    let crash_path = dir.join("crash_log.txt");
    std::panic::set_hook(Box::new(move |info| {
        let _ = fs::write(&crash_path, format!("uWidgets-rs panic:\n{info}\n"));
    }));

    let args: Vec<String> = std::env::args().collect();
    let open_settings_arg = args.iter().any(|a| a == "--settings");
    let is_restarted = args.iter().any(|a| a == "--restarted");

    // Per-Monitor V2 DPI awareness for crisp text & Direct2D rendering
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let ipc_name = to_wide("uWidgetsRs_OpenSettings_IPC");
    let ipc_msg = unsafe { RegisterWindowMessageW(PCWSTR(ipc_name.as_ptr())) };
    unsafe {
        IPC_OPEN_SETTINGS_MSG = ipc_msg;
    }

    // Single-instance guard (skip wait briefly on --restarted)
    if is_restarted {
        std::thread::sleep(std::time::Duration::from_millis(350));
    }
    let mutex_name = to_wide("Local\\uWidgetsRs_SingleInstance_Mutex");
    let _mutex = unsafe { CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr())).ok() };
    let already_running = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
    if already_running {
        let host_c = to_wide(HOST_CLASS);
        let host_t = to_wide(HOST_TITLE);
        if let Ok(existing) =
            unsafe { FindWindowW(PCWSTR(host_c.as_ptr()), PCWSTR(host_t.as_ptr())) }
            && !existing.0.is_null()
        {
            unsafe {
                let _ = PostMessageW(existing, ipc_msg, WPARAM(0), LPARAM(0));
            }
        }
        return;
    }

    // Initialize COM (required for WIC wallpaper decoding & Shell APIs)
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let Ok(gfx) = SharedGraphics::new() else {
        unsafe { CoUninitialize() };
        return;
    };

    // Register all window classes
    register_host_class();
    ui::widget_window::register_class();
    ui::settings_window::register_class();
    ui::edit_window::register_class();
    ui::update_popup::register_class();

    let host_c = to_wide(HOST_CLASS);
    let host_t = to_wide(HOST_TITLE);
    let msg_hwnd = unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            Default::default(),
            PCWSTR(host_c.as_ptr()),
            PCWSTR(host_t.as_ptr()),
            Default::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinst,
            None,
        )
    };
    let Ok(msg_hwnd) = msg_hwnd else {
        unsafe { CoUninitialize() };
        return;
    };

    unsafe {
        let _ = WTSRegisterSessionNotification(msg_hwnd, NOTIFY_FOR_THIS_SESSION);
    }

    let app = Rc::new(RefCell::new(AppState::new(dir, gfx, msg_hwnd)));
    set_global_app(Rc::clone(&app));

    // Add system tray icon
    {
        let mut st = app.borrow_mut();
        st.tray = Some(ui::tray::TrayIcon::new(msg_hwnd));
    }

    // Spawn all saved widgets from layout.json
    let widget_ids: Vec<u64> = app.borrow().layouts.iter().map(|(id, _)| *id).collect();
    for id in &widget_ids {
        ui::widget_window::spawn_widget_window(&app, *id);
    }

    // Start adaptive shared timers + initial weather fetch + 12s one-shot startup auto-update timer
    let auto_check_updates = {
        let mut st = app.borrow_mut();
        st.start_base_timers();
        st.trigger_weather_refresh(false);
        st.settings.updates.auto_check
    };
    if auto_check_updates {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(
                msg_hwnd,
                TIMER_STARTUP_UPDATE,
                12_000,
                None,
            );
        }
    }

    // Show Settings window if `--settings` passed or no widgets exist yet
    if open_settings_arg || widget_ids.is_empty() {
        enqueue_cmd(AppCommand::OpenSettings);
    }

    // Main native Win32 message loop (0% idle CPU!)
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Clean teardown
    {
        let mut st = app.borrow_mut();
        st.tray = None;
        st.widget_windows.clear();
        st.settings_win = None;
        st.edit_win = None;
        st.update_win = None;
    }
    clear_global_app();

    unsafe {
        let _ = WTSUnRegisterSessionNotification(msg_hwnd);
        CoUninitialize();
    }
}

fn register_host_class() {
    let class_w = to_wide(HOST_CLASS);
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(host_wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
    }
}

unsafe extern "system" fn host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ipc_msg = unsafe { IPC_OPEN_SETTINGS_MSG };
    if ipc_msg != 0 && msg == ipc_msg {
        enqueue_cmd(AppCommand::OpenSettings);
        return LRESULT(0);
    }

    match msg {
        WM_APP_TRAY => {
            if let Some(app) = get_global_app() {
                let info = {
                    let st = app.borrow();
                    st.tray
                        .as_ref()
                        .map(|t| (t.hwnd(), st.settings.region.language.clone()))
                };
                if let Some((tray_hwnd, lang)) = info {
                    ui::tray::handle_tray_message(tray_hwnd, lparam, &lang);
                }
            }
            LRESULT(0)
        }
        WM_APP_DISPATCH => {
            if let Some(app) = get_global_app() {
                drain_commands(&app);
            }
            LRESULT(0)
        }
        WM_APP_WEATHER_READY => {
            if let Some(app) = get_global_app() {
                let st = app.borrow();
                st.invalidate_widgets_matching(|w| w.widget_type == "Weather");
                if let Some(sw) = st.settings_win.as_ref()
                    && sw.page == ui::settings_window::NavPage::Weather
                {
                    sw.invalidate();
                }
            }
            LRESULT(0)
        }
        WM_APP_CITY_RESULTS => {
            let res = CITY_SEARCH_SLOT.lock().ok().and_then(|mut g| g.take());
            if let Some(res) = res
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                if let Some(ew) = st.edit_win.as_mut() {
                    match &res {
                        Ok(cities) => {
                            ew.city_status = if cities.is_empty() {
                                Some("No cities found".to_string())
                            } else {
                                Some(format!("Found {} result(s)", cities.len()))
                            };
                            ew.city_results = cities.clone();
                        }
                        Err(e) => {
                            ew.city_status = Some(format!("Error: {e}"));
                        }
                    }
                    ew.invalidate();
                }
                if let Some(sw) = st.settings_win.as_mut() {
                    match res {
                        Ok(cities) => {
                            sw.weather_status = if cities.is_empty() {
                                Some("No matching cities found".to_string())
                            } else {
                                Some(format!(
                                    "Found {} location(s) — click to apply",
                                    cities.len()
                                ))
                            };
                            sw.weather_results = cities;
                        }
                        Err(e) => {
                            sw.weather_status = Some(format!("Search error: {e}"));
                        }
                    }
                    sw.invalidate();
                }
            }
            LRESULT(0)
        }
        WM_APP_DETECT_LOCATION => {
            let res = DETECT_LOC_SLOT.lock().ok().and_then(|mut g| g.take());
            if let Some(res) = res
                && let Some(app) = get_global_app()
            {
                let should_broadcast = {
                    let mut st = app.borrow_mut();
                    match res {
                        Ok(city) => {
                            st.settings.weather.default_city = city.name.clone();
                            st.settings.weather.latitude = city.latitude;
                            st.settings.weather.longitude = city.longitude;
                            st.apply_weather_to_widgets(true, false);
                            st.trigger_weather_refresh(true);
                            if let Some(sw) = st.settings_win.as_mut() {
                                sw.weather_query = city.name.clone();
                                sw.weather_results.clear();
                                sw.weather_status = Some(format!(
                                    "Detected: {} ({:.2}, {:.2})",
                                    city.search_name(),
                                    city.latitude,
                                    city.longitude
                                ));
                                sw.invalidate();
                            }
                            true
                        }
                        Err(e) => {
                            if let Some(sw) = st.settings_win.as_mut() {
                                sw.weather_status = Some(format!("Auto-detect failed: {e}"));
                                sw.invalidate();
                            }
                            false
                        }
                    }
                };
                if should_broadcast {
                    enqueue_cmd(AppCommand::ApplySettingsBroadcast);
                }
            }
            LRESULT(0)
        }
        WM_APP_UPDATE_RESULT => {
            let res = UPDATE_RESULT_SLOT.lock().ok().and_then(|mut g| g.take());
            if let Some((outcome, manual)) = res
                && let Some(app) = get_global_app()
            {
                match outcome {
                    UpdateCheckOutcome::Available(info) => {
                        {
                            let mut st = app.borrow_mut();
                            st.update_status = Some(format!(
                                "Update available: v{} ({})",
                                info.version,
                                info.source.label()
                            ));
                            st.pending_update = Some(info.clone());
                            if let Some(sw) = st.settings_win.as_ref() {
                                sw.invalidate();
                            }
                        }
                        enqueue_cmd(AppCommand::OpenUpdatePopup(info));
                    }
                    UpdateCheckOutcome::StoreUpdatedSilently => {
                        let mut st = app.borrow_mut();
                        st.update_status =
                            Some("Microsoft Store update installed automatically!".to_string());
                        st.pending_update = None;
                        if let Some(sw) = st.settings_win.as_ref() {
                            sw.invalidate();
                        }
                    }
                    UpdateCheckOutcome::DirectUpdatedAndRelaunching(ver) => {
                        {
                            let mut st = app.borrow_mut();
                            st.update_status = Some(format!("Updating to v{ver}…"));
                            if let Some(sw) = st.settings_win.as_ref() {
                                sw.invalidate();
                            }
                        }
                        enqueue_cmd(AppCommand::QuitApp);
                    }
                    UpdateCheckOutcome::UpToDate => {
                        if manual {
                            let mut st = app.borrow_mut();
                            st.update_status =
                                Some(format!("Up to date (v{})", crate::ui::APP_VERSION));
                            st.pending_update = None;
                            if let Some(sw) = st.settings_win.as_ref() {
                                sw.invalidate();
                            }
                        }
                    }
                    UpdateCheckOutcome::Error(err) => {
                        if manual {
                            let mut st = app.borrow_mut();
                            st.update_status = Some(format!("Update check failed: {err}"));
                            if let Some(sw) = st.settings_win.as_ref() {
                                sw.invalidate();
                            }
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_APP_UPDATE_APPLIED => {
            let res = UPDATE_APPLY_SLOT.lock().ok().and_then(|mut g| g.take());
            if let Some(res) = res
                && let Some(app) = get_global_app()
            {
                match res {
                    Ok(ApplyResult::Relaunching) => {
                        enqueue_cmd(AppCommand::QuitApp);
                    }
                    Ok(ApplyResult::StoreInstalled) => {
                        let mut st = app.borrow_mut();
                        st.update_status = Some("Microsoft Store update installed!".to_string());
                        st.pending_update = None;
                        if let Some(sw) = st.settings_win.as_ref() {
                            sw.invalidate();
                        }
                    }
                    Ok(ApplyResult::OpenedExternal) => {
                        let mut st = app.borrow_mut();
                        st.update_status = Some("Opened update installer".to_string());
                        if let Some(sw) = st.settings_win.as_ref() {
                            sw.invalidate();
                        }
                    }
                    Err(e) => {
                        let mut st = app.borrow_mut();
                        st.update_status = Some(format!("Install failed: {e}"));
                        if let Some(sw) = st.settings_win.as_ref() {
                            sw.invalidate();
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                match wparam.0 {
                    TIMER_100MS => {
                        st.invalidate_widgets_matching(|w| {
                            w.widget_type == "Clock"
                                && matches!(
                                    w.sub_type.as_str(),
                                    "AnalogI" | "AnalogII" | "AnalogIII"
                                )
                        });
                    }
                    TIMER_1S => {
                        if st.has_monitor_widgets() {
                            st.monitor.sample();
                        }
                        st.invalidate_widgets_matching(|w| {
                            w.widget_type == "Monitor"
                                || (w.widget_type == "Clock"
                                    && matches!(w.sub_type.as_str(), "Digital" | "Flip"))
                        });
                        if let Some(sw) = st.settings_win.as_ref()
                            && sw.page == ui::settings_window::NavPage::About
                        {
                            sw.invalidate();
                        }
                    }
                    TIMER_5S => {
                        st.invalidate_widgets_matching(|w| w.widget_type == "Clock");
                    }
                    TIMER_1M => {
                        st.invalidate_widgets_matching(|w| {
                            w.widget_type == "Clock"
                                || (w.widget_type == "Weather" && w.sub_type == "SunriseSunset")
                        });
                        // Respects configured Weather refresh interval (5m / 15m / 30m / 60m) with zero wasted HTTP!
                        st.trigger_weather_refresh(false);
                    }
                    TIMER_5M => {
                        st.invalidate_widgets_matching(|w| w.widget_type == "Calendar");
                    }
                    TIMER_1H => {
                        st.trigger_weather_refresh(true);
                        st.update_hours_elapsed = st.update_hours_elapsed.wrapping_add(1);
                        if st.settings.updates.auto_check
                            && st.update_hours_elapsed.is_multiple_of(12)
                        {
                            drop(st);
                            enqueue_cmd(AppCommand::CheckForUpdatesAuto);
                            return LRESULT(0);
                        }
                    }
                    TIMER_STARTUP_UPDATE => {
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(
                                hwnd,
                                TIMER_STARTUP_UPDATE,
                            );
                        }
                        if st.settings.updates.auto_check {
                            drop(st);
                            enqueue_cmd(AppCommand::CheckForUpdatesAuto);
                            return LRESULT(0);
                        }
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            if let Some(app) = get_global_app() {
                {
                    let mut st = app.borrow_mut();
                    st.refresh_system_theme();
                    st.gfx.backdrop.invalidate();
                    for win in st.widget_windows.values_mut() {
                        win.glass_cache.clear();
                    }
                }
                enqueue_cmd(AppCommand::ApplySettingsBroadcast);
            }
            LRESULT(0)
        }
        WM_WTSSESSION_CHANGE => {
            if wparam.0 as u32 == WTS_SESSION_UNLOCK
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                if st.has_monitor_widgets() {
                    st.monitor.sample();
                }
                st.invalidate_all_widgets();
                st.trigger_weather_refresh(false);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
