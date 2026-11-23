use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GetClientRect, GetSystemMetrics,
    IDC_ARROW, LoadCursorW, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, ShowWindow,
    WM_CLOSE, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_CAPTION,
    WS_EX_NOREDIRECTIONBITMAP, WS_SYSMENU, WS_VISIBLE,
};
use windows_core::PCWSTR;

use crate::i18n::tr;
use crate::platform::graphics::{
    WindowSurface, draw_round_rect, draw_text, fill_round_rect, rect, rgba,
};
use crate::platform::system::{create_app_icon, lparam_point, open_url, to_wide};
use crate::ui::{AppCommand, AppState, enqueue_cmd, get_global_app, settings_window};
use crate::updater::UpdateInfo;

const CLASS_NAME: &str = "uWidgetsRsUpdatePopup";

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnAction {
    Install,
    OpenPage,
    Later,
    Skip,
}

pub struct UpdatePopup {
    pub hwnd: HWND,
    pub surface: WindowSurface,
    pub info: UpdateInfo,
    hover: Option<BtnAction>,
    buttons: Vec<(D2D_RECT_F, BtnAction)>,
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

pub fn open_popup(app: &Rc<RefCell<AppState>>, info: UpdateInfo) {
    let existing = { app.borrow().update_win.as_ref().map(|u| u.hwnd) };
    if let Some(existing) = existing {
        {
            let mut st = app.borrow_mut();
            if let Some(up) = st.update_win.as_mut() {
                up.info = info;
            }
        }
        unsafe {
            let _ = ShowWindow(existing, SW_SHOW);
            let _ = InvalidateRect(existing, None, false);
        }
        return;
    }

    let (title, is_dark, gfx) = {
        let st = app.borrow();
        (
            tr(&st.settings.region.language, "Update_SoftwareUpdate").to_string(),
            st.resolved_theme().is_dark,
            Rc::clone(&st.gfx),
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
    let w = (460.0 * dpi_scale).round() as i32;
    let h = (224.0 * dpi_scale).round() as i32;
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
        app.borrow_mut().update_win = Some(UpdatePopup {
            hwnd,
            surface,
            info,
            hover: None,
            buttons: Vec::new(),
        });
        unsafe {
            let _ = InvalidateRect(hwnd, None, false);
        }
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
                let theme = st.resolved_theme();
                let lang = st.settings.region.language.clone();
                let font = st.settings.theme.font_family.clone();
                if let Some(up) = st.update_win.as_mut() {
                    let w = up.surface.width as f32 * 96.0 / up.surface.dpi;
                    let h = up.surface.height as f32 * 96.0 / up.surface.dpi;
                    up.buttons.clear();
                    up.surface.begin();

                    let bg = if theme.is_dark {
                        rgba(0.13, 0.13, 0.14, 1.0)
                    } else {
                        rgba(0.96, 0.96, 0.97, 1.0)
                    };
                    let fg = if theme.is_dark {
                        rgba(1.0, 1.0, 1.0, 0.95)
                    } else {
                        rgba(0.1, 0.1, 0.1, 0.95)
                    };
                    let muted = if theme.is_dark {
                        rgba(0.72, 0.72, 0.75, 1.0)
                    } else {
                        rgba(0.38, 0.38, 0.42, 1.0)
                    };
                    let (ar, ag, ab) = theme.accent_rgb;
                    let accent = rgba(ar, ag, ab, 1.0);

                    fill_round_rect(&up.surface.dc, rect(0.0, 0.0, w, h), 0.0, bg);

                    let heading = tr(&lang, "Update_SoftwareUpdate");
                    let detail_line = up
                        .info
                        .notes
                        .as_deref()
                        .and_then(|n| n.lines().find(|l| !l.trim().is_empty()))
                        .map(|l| l.trim().trim_start_matches('#').trim().to_string())
                        .unwrap_or_else(|| {
                            if up.info.download_url.is_some()
                                || up.info.source == crate::updater::UpdateSource::MicrosoftStore
                            {
                                "Automatic in-place update ready".to_string()
                            } else {
                                "Download available on release page".to_string()
                            }
                        });
                    let body = format!(
                        "{} ({})\n{detail_line}",
                        tr(&lang, "Update_UpdateFormat").replace("{0}", &up.info.version),
                        up.info.source.label(),
                    );

                    draw_text(
                        &up.surface.gfx,
                        &up.surface.dc,
                        heading,
                        rect(22.0, 18.0, w - 22.0, 44.0),
                        &font,
                        17.0,
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                        fg,
                        false,
                    );
                    draw_text(
                        &up.surface.gfx,
                        &up.surface.dc,
                        &body,
                        rect(22.0, 50.0, w - 22.0, 118.0),
                        &font,
                        12.5,
                        DWRITE_FONT_WEIGHT_MEDIUM,
                        DWRITE_TEXT_ALIGNMENT_LEADING,
                        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                        muted,
                        true,
                    );

                    let btn_y = h - 52.0;
                    let btn_h = 34.0;
                    let r_inst = rect(w - 132.0, btn_y, w - 20.0, btn_y + btn_h);
                    let r_page = rect(w - 226.0, btn_y, w - 140.0, btn_y + btn_h);
                    let r_later = rect(w - 306.0, btn_y, w - 234.0, btn_y + btn_h);
                    let r_skip = rect(20.0, btn_y, 134.0, btn_y + btn_h);

                    for (r, action, lbl, primary) in [
                        (r_inst, BtnAction::Install, "Install Now", true),
                        (r_page, BtnAction::OpenPage, "Details", false),
                        (r_later, BtnAction::Later, tr(&lang, "Update_Later"), false),
                        (r_skip, BtnAction::Skip, "Skip", false),
                    ] {
                        let hovered = up.hover == Some(action);
                        let fill = if primary {
                            if hovered {
                                rgba(
                                    (ar + 0.08).min(1.0),
                                    (ag + 0.08).min(1.0),
                                    (ab + 0.08).min(1.0),
                                    1.0,
                                )
                            } else {
                                accent
                            }
                        } else if theme.is_dark {
                            rgba(1.0, 1.0, 1.0, if hovered { 0.14 } else { 0.08 })
                        } else {
                            rgba(0.0, 0.0, 0.0, if hovered { 0.10 } else { 0.05 })
                        };
                        fill_round_rect(&up.surface.dc, r, 7.0, fill);
                        if !primary {
                            draw_round_rect(
                                &up.surface.dc,
                                r,
                                7.0,
                                if theme.is_dark {
                                    rgba(1.0, 1.0, 1.0, 0.12)
                                } else {
                                    rgba(0.0, 0.0, 0.0, 0.12)
                                },
                                1.0,
                            );
                        }
                        draw_text(
                            &up.surface.gfx,
                            &up.surface.dc,
                            lbl,
                            r,
                            &font,
                            12.5,
                            DWRITE_FONT_WEIGHT_SEMI_BOLD,
                            DWRITE_TEXT_ALIGNMENT_CENTER,
                            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                            if primary {
                                rgba(1.0, 1.0, 1.0, 1.0)
                            } else {
                                fg
                            },
                            false,
                        );
                        up.buttons.push((r, action));
                    }
                    let _ = up.surface.end();
                }
            }
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                if let Some(up) = st.update_win.as_mut() {
                    let mut rc = RECT::default();
                    let dpi = unsafe {
                        let _ = GetClientRect(hwnd, &mut rc);
                        (GetDpiForWindow(hwnd) as f32).max(96.0)
                    };
                    let _ = up.surface.resize(
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
        WM_MOUSEMOVE => {
            let (px, py) = lparam_point(lparam);
            if let Some(app) = get_global_app() {
                let mut st = app.borrow_mut();
                if let Some(up) = st.update_win.as_mut() {
                    let dip_x = px as f32 * 96.0 / up.surface.dpi;
                    let dip_y = py as f32 * 96.0 / up.surface.dpi;
                    let new_hover = up
                        .buttons
                        .iter()
                        .find(|(r, _)| {
                            dip_x >= r.left
                                && dip_x <= r.right
                                && dip_y >= r.top
                                && dip_y <= r.bottom
                        })
                        .map(|(_, a)| *a);
                    if new_hover != up.hover {
                        up.hover = new_hover;
                        unsafe {
                            let _ = InvalidateRect(hwnd, None, false);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (px, py) = lparam_point(lparam);
            let hit = if let Some(app) = get_global_app() {
                let st = app.borrow();
                st.update_win.as_ref().and_then(|up| {
                    let dip_x = px as f32 * 96.0 / up.surface.dpi;
                    let dip_y = py as f32 * 96.0 / up.surface.dpi;
                    up.buttons
                        .iter()
                        .find(|(r, _)| {
                            dip_x >= r.left
                                && dip_x <= r.right
                                && dip_y >= r.top
                                && dip_y <= r.bottom
                        })
                        .map(|(_, a)| (*a, up.info.clone()))
                })
            } else {
                None
            };
            if let Some((action, info)) = hit {
                match action {
                    BtnAction::Install => {
                        enqueue_cmd(AppCommand::InstallPendingUpdate(info));
                        enqueue_cmd(AppCommand::CloseUpdatePopup);
                    }
                    BtnAction::OpenPage => {
                        open_url(&info.release_page);
                        enqueue_cmd(AppCommand::CloseUpdatePopup);
                    }
                    BtnAction::Later => {
                        enqueue_cmd(AppCommand::CloseUpdatePopup);
                    }
                    BtnAction::Skip => {
                        if let Some(app) = get_global_app() {
                            let mut st = app.borrow_mut();
                            st.settings.ignore_update = Some(info.version);
                            st.save_settings();
                        }
                        enqueue_cmd(AppCommand::CloseUpdatePopup);
                    }
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            enqueue_cmd(AppCommand::CloseUpdatePopup);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
