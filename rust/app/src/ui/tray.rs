use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos, HICON, MF_SEPARATOR,
    MF_STRING, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
};
use windows_core::PCWSTR;

use crate::i18n::tr;
use crate::platform::system::{create_app_icon, to_wide};
use crate::ui::{AppCommand, WM_APP_TRAY, enqueue_cmd};

const TRAY_ID: u32 = 1;
const CMD_OPEN_SETTINGS: u32 = 1001;
const CMD_RELOAD: u32 = 1002;
const CMD_EXIT: u32 = 1003;

pub struct TrayIcon {
    hwnd: HWND,
    icon: Option<HICON>,
    added: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND) -> Self {
        let icon = create_app_icon(16);
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE
                | NIF_TIP
                | if icon.is_some() {
                    NIF_ICON
                } else {
                    Default::default()
                },
            uCallbackMessage: WM_APP_TRAY,
            hIcon: icon.unwrap_or_default(),
            ..Default::default()
        };
        let tip = to_wide("uWidgets (Rust)");
        for (dst, &src) in nid.szTip.iter_mut().zip(tip.iter()) {
            *dst = src;
        }
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &nid).as_bool() };
        Self { hwnd, icon, added }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

/// Handles tray icon callback messages without holding any `AppState` borrow across `TrackPopupMenu`.
pub fn handle_tray_message(hwnd: HWND, lparam: LPARAM, lang: &str) {
    let event = (lparam.0 & 0xFFFF) as u32;
    match event {
        WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
            enqueue_cmd(AppCommand::OpenSettings);
        }
        WM_RBUTTONUP => {
            show_tray_menu(hwnd, lang);
        }
        _ => {}
    }
}

fn show_tray_menu(hwnd: HWND, lang: &str) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let s_open = to_wide(tr(lang, "Widget_OpenSettings"));
        let s_reload = to_wide(tr(lang, "Settings_General_Restart_Action"));
        let s_exit = to_wide(tr(lang, "Settings_General_Exit_Action"));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_OPEN_SETTINGS as usize,
            PCWSTR(s_open.as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            CMD_RELOAD as usize,
            PCWSTR(s_reload.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT as usize, PCWSTR(s_exit.as_ptr()));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        )
        .0 as u32;
        let _ = DestroyMenu(menu);

        match chosen {
            CMD_OPEN_SETTINGS => enqueue_cmd(AppCommand::OpenSettings),
            CMD_RELOAD => enqueue_cmd(AppCommand::ReloadAllWidgets),
            CMD_EXIT => enqueue_cmd(AppCommand::QuitApp),
            _ => {}
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        if self.added {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ID,
                ..Default::default()
            };
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
        if let Some(ic) = self.icon {
            unsafe {
                let _ = DestroyIcon(ic);
            }
        }
    }
}
