#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use flate2::read::ZlibDecoder;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW,
    CreatePen, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, GetDC, GetDeviceCaps, HGDIOBJ,
    InvalidateRect, LOGPIXELSX, PAINTSTRUCT, PS_SOLID, ReleaseDC, RoundRect, SRCCOPY, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IPersistFile,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateIconFromResourceEx, CreateWindowExW, DI_NORMAL, DefWindowProcW,
    DispatchMessageW, DrawIconEx, GetClientRect, GetMessageW, GetSystemMetrics, HICON, IDC_ARROW,
    IDC_HAND, LR_DEFAULTCOLOR, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
    SM_CYSCREEN, SW_SHOW, SetCursor, SetForegroundWindow, ShowWindow, TranslateMessage, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WNDCLASSEXW, WS_CAPTION,
    WS_MINIMIZEBOX, WS_SYSMENU, WS_VISIBLE,
};
use windows_core::{Interface, PCWSTR};

const APP_NAME: &str = "uWidgets (Rust Edition)";
const APP_VERSION: &str = "1.0.0";
const REPO_URL: &str = "https://github.com/nabil24024004/uWidgets-Rust-Rebuild";
const CLASS_NAME: &str = "uWidgetsRsSetupWizard";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const PAYLOAD_ZLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uwidgets_payload.zlib"));
const APP_ICO_BYTES: &[u8] = include_bytes!("../../../assets/uWidgets.ico");

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16))
}

fn create_icon(size: i32) -> Option<HICON> {
    if APP_ICO_BYTES.len() < 6 {
        return None;
    }
    let count = u16::from_le_bytes([APP_ICO_BYTES[4], APP_ICO_BYTES[5]]) as usize;
    let mut best: Option<(usize, usize, i32)> = None;
    for i in 0..count {
        let entry = 6 + i * 16;
        if entry + 16 > APP_ICO_BYTES.len() {
            break;
        }
        let w = if APP_ICO_BYTES[entry] == 0 {
            256
        } else {
            i32::from(APP_ICO_BYTES[entry])
        };
        let bytes_len = u32::from_le_bytes([
            APP_ICO_BYTES[entry + 8],
            APP_ICO_BYTES[entry + 9],
            APP_ICO_BYTES[entry + 10],
            APP_ICO_BYTES[entry + 11],
        ]) as usize;
        let offset = u32::from_le_bytes([
            APP_ICO_BYTES[entry + 12],
            APP_ICO_BYTES[entry + 13],
            APP_ICO_BYTES[entry + 14],
            APP_ICO_BYTES[entry + 15],
        ]) as usize;
        let diff = (w - size).abs();
        if offset + bytes_len <= APP_ICO_BYTES.len() && best.is_none_or(|(_, _, bd)| diff < bd) {
            best = Some((offset, bytes_len, diff));
        }
    }
    let (off, len, _) = best?;
    unsafe {
        CreateIconFromResourceEx(
            &APP_ICO_BYTES[off..off + len],
            true,
            0x0003_0000,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
        .ok()
    }
}

fn default_install_dir() -> PathBuf {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("uWidgets")
    } else {
        PathBuf::from("C:\\uWidgets")
    }
}

fn start_menu_shortcut_path() -> Option<PathBuf> {
    env::var("APPDATA")
        .ok()
        .map(|a| PathBuf::from(a).join("Microsoft\\Windows\\Start Menu\\Programs\\uWidgets.lnk"))
}

fn desktop_shortcut_path() -> Option<PathBuf> {
    env::var("USERPROFILE")
        .ok()
        .map(|u| PathBuf::from(u).join("Desktop\\uWidgets.lnk"))
}

fn stop_running_app() {
    let _ = Command::new("taskkill")
        .args(["/IM", "uwidgets-rs.exe", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    thread::sleep(Duration::from_millis(280));
}

fn create_shortcut(
    lnk_path: &Path,
    target_exe: &Path,
    work_dir: &Path,
    description: &str,
) -> Result<(), String> {
    if let Some(parent) = lnk_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance IShellLinkW: {e}"))?;
        let t_w = to_wide(&target_exe.to_string_lossy());
        let w_w = to_wide(&work_dir.to_string_lossy());
        let d_w = to_wide(description);
        link.SetPath(PCWSTR(t_w.as_ptr()))
            .map_err(|e| format!("SetPath: {e}"))?;
        link.SetWorkingDirectory(PCWSTR(w_w.as_ptr()))
            .map_err(|e| format!("SetWorkingDirectory: {e}"))?;
        link.SetDescription(PCWSTR(d_w.as_ptr()))
            .map_err(|e| format!("SetDescription: {e}"))?;
        link.SetIconLocation(PCWSTR(t_w.as_ptr()), 0)
            .map_err(|e| format!("SetIconLocation: {e}"))?;
        let pf: IPersistFile = link.cast().map_err(|e| format!("cast IPersistFile: {e}"))?;
        let p_w = to_wide(&lnk_path.to_string_lossy());
        pf.Save(PCWSTR(p_w.as_ptr()), true)
            .map_err(|e| format!("IPersistFile::Save: {e}"))?;
    }
    Ok(())
}

fn reg_set_sz(hkey: HKEY, name: &str, val: &str) {
    let nw = to_wide(name);
    let vw = to_wide(val);
    unsafe {
        let bytes = std::slice::from_raw_parts(vw.as_ptr().cast::<u8>(), vw.len() * 2);
        let _ = RegSetValueExW(hkey, PCWSTR(nw.as_ptr()), 0, REG_SZ, Some(bytes));
    }
}

fn reg_set_dword(hkey: HKEY, name: &str, val: u32) {
    let nw = to_wide(name);
    unsafe {
        let bytes = std::slice::from_raw_parts((&val as *const u32).cast::<u8>(), 4);
        let _ = RegSetValueExW(hkey, PCWSTR(nw.as_ptr()), 0, REG_DWORD, Some(bytes));
    }
}

fn set_startup_entry(exe_path: Option<&Path>) {
    let subkey = to_wide("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
    let val_name = to_wide("uWidgets");
    let mut hkey = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .is_ok()
        {
            match exe_path {
                Some(p) => {
                    let quoted = format!("\"{}\"", p.to_string_lossy());
                    reg_set_sz(hkey, "uWidgets", &quoted);
                }
                None => {
                    let _ = RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr()));
                }
            }
            let _ = RegCloseKey(hkey);
        }
    }
}

fn register_uninstaller(install_dir: &Path, exe_path: &Path, uninst_path: &Path, size_kb: u32) {
    let subkey = to_wide("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\uWidgets-rs");
    let mut hkey = HKEY::default();
    unsafe {
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .is_ok()
        {
            reg_set_sz(hkey, "DisplayName", APP_NAME);
            reg_set_sz(hkey, "DisplayVersion", APP_VERSION);
            reg_set_sz(hkey, "Publisher", "Azwad Abrar Nabil (uWidgets-rs)");
            reg_set_sz(hkey, "InstallLocation", &install_dir.to_string_lossy());
            reg_set_sz(hkey, "DisplayIcon", &format!("{},0", exe_path.display()));
            reg_set_sz(
                hkey,
                "UninstallString",
                &format!("\"{}\" --uninstall", uninst_path.display()),
            );
            reg_set_sz(
                hkey,
                "QuietUninstallString",
                &format!("\"{}\" --uninstall /S", uninst_path.display()),
            );
            reg_set_sz(hkey, "URLInfoAbout", REPO_URL);
            reg_set_dword(hkey, "EstimatedSize", size_kb);
            reg_set_dword(hkey, "NoModify", 1);
            reg_set_dword(hkey, "NoRepair", 1);
            let _ = RegCloseKey(hkey);
        }
    }
}

fn remove_uninstaller_reg() {
    let subkey = to_wide("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\uWidgets-rs");
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr()));
    }
}

struct InstallOptions {
    install_dir: PathBuf,
    uninstall_mode: bool,
    silent: bool,
    start_menu: bool,
    desktop_shortcut: bool,
    run_on_startup: bool,
    launch_after: bool,
    remove_user_data: bool,
    skip_system_hooks: bool,
}

fn run_install(opts: &InstallOptions) -> Result<String, String> {
    if !opts.skip_system_hooks {
        stop_running_app();
    }
    fs::create_dir_all(&opts.install_dir)
        .map_err(|e| format!("Failed to create install folder: {e}"))?;

    let mut decoder = ZlibDecoder::new(PAYLOAD_ZLIB);
    let mut app_bytes = Vec::with_capacity(4 * 1024 * 1024);
    decoder
        .read_to_end(&mut app_bytes)
        .map_err(|e| format!("Failed to decompress payload: {e}"))?;

    let exe_path = opts.install_dir.join("uwidgets-rs.exe");
    fs::write(&exe_path, &app_bytes)
        .map_err(|e| format!("Failed to write {}: {e}", exe_path.display()))?;

    let uninst_path = opts.install_dir.join("uWidgets-Uninstall.exe");
    if let Ok(self_exe) = env::current_exe()
        && self_exe != uninst_path
    {
        let _ = fs::copy(&self_exe, &uninst_path);
    }

    if !opts.skip_system_hooks {
        let size_kb = (app_bytes.len() / 1024).max(1) as u32;
        register_uninstaller(&opts.install_dir, &exe_path, &uninst_path, size_kb);

        if opts.start_menu
            && let Some(lnk) = start_menu_shortcut_path()
        {
            let _ = create_shortcut(
                &lnk,
                &exe_path,
                &opts.install_dir,
                "uWidgets (Rust Edition) Desktop Widgets",
            );
        }
        if opts.desktop_shortcut
            && let Some(lnk) = desktop_shortcut_path()
        {
            let _ = create_shortcut(
                &lnk,
                &exe_path,
                &opts.install_dir,
                "uWidgets (Rust Edition) Desktop Widgets",
            );
        }
        if opts.run_on_startup {
            set_startup_entry(Some(&exe_path));
        }
        if opts.launch_after {
            let _ = Command::new(&exe_path)
                .arg("--settings")
                .current_dir(&opts.install_dir)
                .spawn();
        }
    }

    Ok(format!(
        "Installed uWidgets v{APP_VERSION} to {}",
        opts.install_dir.display()
    ))
}

fn run_uninstall(opts: &InstallOptions) -> Result<String, String> {
    if !opts.skip_system_hooks {
        stop_running_app();
        if let Some(lnk) = start_menu_shortcut_path() {
            let _ = fs::remove_file(lnk);
        }
        if let Some(lnk) = desktop_shortcut_path() {
            let _ = fs::remove_file(lnk);
        }
        set_startup_entry(None);
        remove_uninstaller_reg();
    }

    let _ = fs::remove_file(opts.install_dir.join("uwidgets-rs.exe"));
    let _ = fs::remove_file(opts.install_dir.join("weatherCache.json"));
    let _ = fs::remove_file(opts.install_dir.join("crash_log.txt"));
    let _ = fs::remove_file(opts.install_dir.join("weather_crash_log.txt"));
    if opts.remove_user_data {
        let _ = fs::remove_file(opts.install_dir.join("appSettings.json"));
        let _ = fs::remove_file(opts.install_dir.join("layout.json"));
    }

    // Schedule removal of self if running from inside install_dir
    if let Ok(self_exe) = env::current_exe()
        && self_exe.starts_with(&opts.install_dir)
    {
        let script = format!(
            "ping 127.0.0.1 -n 2 > nul & del /F /Q \"{}\" & rmdir \"{}\"",
            self_exe.display(),
            opts.install_dir.display()
        );
        let _ = Command::new("cmd")
            .args(["/C", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    } else {
        let _ = fs::remove_file(opts.install_dir.join("uWidgets-Uninstall.exe"));
        let _ = fs::remove_dir(&opts.install_dir);
    }

    Ok("uWidgets has been cleanly uninstalled.".to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionId {
    ToggleStartMenu,
    ToggleDesktop,
    ToggleStartup,
    ToggleLaunch,
    ToggleRemoveData,
    PrimaryButton,
    CancelButton,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WizardPhase {
    Ready,
    Done,
    Failed,
}

struct WizardState {
    opts: InstallOptions,
    phase: WizardPhase,
    status_msg: String,
    logo_icon: Option<HICON>,
    hits: Vec<(RECT, ActionId)>,
    hovered: Option<ActionId>,
}

thread_local! {
    static STATE: RefCell<Option<WizardState>> = const { RefCell::new(None) };
}

fn s(v: f32, scale: f32) -> i32 {
    (v * scale).round() as i32
}

fn make_font(height: i32, weight: i32) -> windows::Win32::Graphics::Gdi::HFONT {
    let face = to_wide("Segoe UI Variable");
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1,
            0,
            0,
            u32::from(CLEARTYPE_QUALITY.0),
            0,
            PCWSTR(face.as_ptr()),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_label(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    r: RECT,
    size: i32,
    weight: i32,
    color: COLORREF,
    align: u32,
) {
    let mut wide: Vec<u16> = OsStr::new(text).encode_wide().collect();
    let mut rc = r;
    unsafe {
        let font = make_font(size, weight);
        let old = SelectObject(hdc, HGDIOBJ(font.0));
        SetTextColor(hdc, color);
        SetBkMode(hdc, TRANSPARENT);
        DrawTextW(
            hdc,
            &mut wide,
            &mut rc,
            windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT(
                align | DT_VCENTER.0 | DT_SINGLELINE.0 | DT_END_ELLIPSIS.0,
            ),
        );
        SelectObject(hdc, old);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
}

fn fill_rounded(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    r: RECT,
    radius: i32,
    fill: COLORREF,
    border: COLORREF,
) {
    unsafe {
        let brush = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, border);
        let ob = SelectObject(hdc, HGDIOBJ(brush.0));
        let op = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        let _ = DeleteObject(HGDIOBJ(brush.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
}

fn paint_wizard(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let mut cr = RECT::default();
    let dpi = unsafe {
        let _ = GetClientRect(hwnd, &mut cr);
        (GetDpiForWindow(hwnd) as f32).max(96.0)
    };
    let scale = dpi / 96.0;
    let cw = (cr.right - cr.left).max(1);
    let ch = (cr.bottom - cr.top).max(1);

    unsafe {
        let mem_dc = CreateCompatibleDC(hdc);
        let mem_bmp = CreateCompatibleBitmap(hdc, cw, ch);
        let old_bmp = SelectObject(mem_dc, HGDIOBJ(mem_bmp.0));

        // Dark Apple / Win11 canvas
        let bg = CreateSolidBrush(rgb(24, 26, 31));
        FillRect(mem_dc, &cr, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));

        STATE.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let Some(st) = borrow.as_mut() else { return };
            st.hits.clear();

            let pad = s(24.0, scale);
            let card_bg = rgb(36, 39, 46);
            let card_bdr = rgb(58, 63, 74);
            let accent = rgb(36, 128, 240);
            let accent_hi = rgb(58, 146, 255);
            let fg = rgb(245, 247, 252);
            let muted = rgb(164, 170, 184);

            // Header with embedded multi-res icon
            let ico_sz = s(52.0, scale);
            if let Some(icon) = st.logo_icon {
                let _ = DrawIconEx(
                    mem_dc,
                    pad,
                    s(20.0, scale),
                    icon,
                    ico_sz,
                    ico_sz,
                    0,
                    None,
                    DI_NORMAL,
                );
            }
            let title = if st.opts.uninstall_mode {
                "Uninstall uWidgets"
            } else {
                APP_NAME
            };
            let subtitle = if st.opts.uninstall_mode {
                format!("Version {APP_VERSION}  •  Clean per-user removal")
            } else {
                format!("Version {APP_VERSION}  •  Liquid Glass Desktop Widgets Setup")
            };
            draw_label(
                mem_dc,
                title,
                RECT {
                    left: pad + ico_sz + s(16.0, scale),
                    top: s(20.0, scale),
                    right: cw - pad,
                    bottom: s(48.0, scale),
                },
                s(20.0, scale),
                700,
                fg,
                DT_LEFT.0,
            );
            draw_label(
                mem_dc,
                &subtitle,
                RECT {
                    left: pad + ico_sz + s(16.0, scale),
                    top: s(48.0, scale),
                    right: cw - pad,
                    bottom: s(70.0, scale),
                },
                s(12.5, scale),
                400,
                muted,
                DT_LEFT.0,
            );

            // Install path card
            let path_r = RECT {
                left: pad,
                top: s(88.0, scale),
                right: cw - pad,
                bottom: s(142.0, scale),
            };
            fill_rounded(mem_dc, path_r, s(12.0, scale), card_bg, card_bdr);
            draw_label(
                mem_dc,
                if st.opts.uninstall_mode {
                    "Installed Location"
                } else {
                    "Install Destination (No Admin UAC Required)"
                },
                RECT {
                    left: path_r.left + s(14.0, scale),
                    top: path_r.top + s(7.0, scale),
                    right: path_r.right - s(14.0, scale),
                    bottom: path_r.top + s(24.0, scale),
                },
                s(11.5, scale),
                600,
                muted,
                DT_LEFT.0,
            );
            draw_label(
                mem_dc,
                &st.opts.install_dir.to_string_lossy(),
                RECT {
                    left: path_r.left + s(14.0, scale),
                    top: path_r.top + s(25.0, scale),
                    right: path_r.right - s(14.0, scale),
                    bottom: path_r.bottom - s(7.0, scale),
                },
                s(13.0, scale),
                600,
                fg,
                DT_LEFT.0,
            );

            // Options or Completion Status
            let mut y = s(156.0, scale);
            if st.phase == WizardPhase::Ready {
                let options: Vec<(&str, bool, ActionId)> = if st.opts.uninstall_mode {
                    vec![(
                        "Also delete saved widget layout & settings (appSettings.json)",
                        st.opts.remove_user_data,
                        ActionId::ToggleRemoveData,
                    )]
                } else {
                    vec![
                        (
                            "Create Start Menu shortcut",
                            st.opts.start_menu,
                            ActionId::ToggleStartMenu,
                        ),
                        (
                            "Create Desktop shortcut",
                            st.opts.desktop_shortcut,
                            ActionId::ToggleDesktop,
                        ),
                        (
                            "Start uWidgets automatically with Windows",
                            st.opts.run_on_startup,
                            ActionId::ToggleStartup,
                        ),
                        (
                            "Launch uWidgets after installation",
                            st.opts.launch_after,
                            ActionId::ToggleLaunch,
                        ),
                    ]
                };

                let row_h = s(38.0, scale);
                let row_gap = s(8.0, scale);
                for (label, checked, act) in options {
                    let rr = RECT {
                        left: pad,
                        top: y,
                        right: cw - pad,
                        bottom: y + row_h,
                    };
                    let hov = st.hovered == Some(act);
                    fill_rounded(
                        mem_dc,
                        rr,
                        s(10.0, scale),
                        if hov { rgb(44, 48, 57) } else { card_bg },
                        if checked { rgb(46, 98, 168) } else { card_bdr },
                    );
                    let cb_sz = s(18.0, scale);
                    let cb_r = RECT {
                        left: rr.left + s(12.0, scale),
                        top: rr.top + (row_h - cb_sz) / 2,
                        right: rr.left + s(12.0, scale) + cb_sz,
                        bottom: rr.top + (row_h + cb_sz) / 2,
                    };
                    fill_rounded(
                        mem_dc,
                        cb_r,
                        s(5.0, scale),
                        if checked { accent } else { rgb(26, 28, 34) },
                        if checked { accent } else { rgb(90, 96, 112) },
                    );
                    if checked {
                        draw_label(mem_dc, "✓", cb_r, s(12.5, scale), 700, fg, DT_CENTER.0);
                    }
                    draw_label(
                        mem_dc,
                        label,
                        RECT {
                            left: cb_r.right + s(12.0, scale),
                            top: rr.top,
                            right: rr.right - s(12.0, scale),
                            bottom: rr.bottom,
                        },
                        s(13.0, scale),
                        500,
                        fg,
                        DT_LEFT.0,
                    );
                    st.hits.push((rr, act));
                    y += row_h + row_gap;
                }
            } else {
                let status_r = RECT {
                    left: pad,
                    top: y,
                    right: cw - pad,
                    bottom: y + s(126.0, scale),
                };
                let ok = st.phase == WizardPhase::Done;
                fill_rounded(
                    mem_dc,
                    status_r,
                    s(12.0, scale),
                    card_bg,
                    if ok { rgb(22, 163, 74) } else { rgb(220, 38, 38) },
                );
                draw_label(
                    mem_dc,
                    if ok {
                        if st.opts.uninstall_mode {
                            "Uninstall Complete"
                        } else {
                            "Installation Complete!"
                        }
                    } else {
                        "Setup Encountered an Error"
                    },
                    RECT {
                        left: status_r.left + s(16.0, scale),
                        top: status_r.top + s(20.0, scale),
                        right: status_r.right - s(16.0, scale),
                        bottom: status_r.top + s(50.0, scale),
                    },
                    s(16.5, scale),
                    700,
                    if ok {
                        rgb(74, 222, 128)
                    } else {
                        rgb(248, 113, 113)
                    },
                    DT_LEFT.0,
                );
                draw_label(
                    mem_dc,
                    &st.status_msg,
                    RECT {
                        left: status_r.left + s(16.0, scale),
                        top: status_r.top + s(54.0, scale),
                        right: status_r.right - s(16.0, scale),
                        bottom: status_r.top + s(86.0, scale),
                    },
                    s(12.5, scale),
                    400,
                    fg,
                    DT_LEFT.0,
                );
                if ok && !st.opts.uninstall_mode {
                    draw_label(
                        mem_dc,
                        "Tip: Right-click the uWidgets tray icon anytime to customize Liquid Glass & Weather.",
                        RECT {
                            left: status_r.left + s(16.0, scale),
                            top: status_r.top + s(86.0, scale),
                            right: status_r.right - s(16.0, scale),
                            bottom: status_r.bottom - s(12.0, scale),
                        },
                        s(11.5, scale),
                        400,
                        muted,
                        DT_LEFT.0,
                    );
                }
            }

            // Bottom action buttons
            let btn_h = s(38.0, scale);
            let btn_y = ch - pad - btn_h;
            let prim_w = s(172.0, scale);
            let prim_r = RECT {
                left: cw - pad - prim_w,
                top: btn_y,
                right: cw - pad,
                bottom: btn_y + btn_h,
            };
            let prim_lbl = match st.phase {
                WizardPhase::Ready if st.opts.uninstall_mode => "Uninstall",
                WizardPhase::Ready => "Install v1.0.0",
                WizardPhase::Done | WizardPhase::Failed => "Finish",
            };
            let prim_col = if st.opts.uninstall_mode && st.phase == WizardPhase::Ready {
                if st.hovered == Some(ActionId::PrimaryButton) {
                    rgb(239, 68, 68)
                } else {
                    rgb(210, 42, 42)
                }
            } else if st.hovered == Some(ActionId::PrimaryButton) {
                accent_hi
            } else {
                accent
            };
            fill_rounded(mem_dc, prim_r, s(10.0, scale), prim_col, prim_col);
            draw_label(mem_dc, prim_lbl, prim_r, s(13.5, scale), 600, fg, DT_CENTER.0);
            st.hits.push((prim_r, ActionId::PrimaryButton));

            if st.phase == WizardPhase::Ready {
                let cancel_w = s(96.0, scale);
                let cancel_r = RECT {
                    left: prim_r.left - s(10.0, scale) - cancel_w,
                    top: btn_y,
                    right: prim_r.left - s(10.0, scale),
                    bottom: btn_y + btn_h,
                };
                fill_rounded(
                    mem_dc,
                    cancel_r,
                    s(10.0, scale),
                    if st.hovered == Some(ActionId::CancelButton) {
                        rgb(52, 56, 66)
                    } else {
                        card_bg
                    },
                    card_bdr,
                );
                draw_label(mem_dc, "Cancel", cancel_r, s(13.0, scale), 600, fg, DT_CENTER.0);
                st.hits.push((cancel_r, ActionId::CancelButton));
            }
        });

        let _ = BitBlt(hdc, 0, 0, cw, ch, mem_dc, 0, 0, SRCCOPY);
        SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(HGDIOBJ(mem_bmp.0));
        let _ = DeleteDC(mem_dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn pt_in_rect(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint_wizard(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let (changed, over_any) = STATE.with(|cell| {
                let mut borrow = cell.borrow_mut();
                let Some(st) = borrow.as_mut() else {
                    return (false, false);
                };
                let found = st
                    .hits
                    .iter()
                    .find(|(r, _)| pt_in_rect(*r, x, y))
                    .map(|(_, a)| *a);
                let ch = found != st.hovered;
                st.hovered = found;
                (ch, found.is_some())
            });
            unsafe {
                if let Ok(cur) = LoadCursorW(None, if over_any { IDC_HAND } else { IDC_ARROW }) {
                    SetCursor(cur);
                }
                if changed {
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let quit = STATE.with(|cell| {
                let mut borrow = cell.borrow_mut();
                let Some(st) = borrow.as_mut() else {
                    return false;
                };
                let hit = st
                    .hits
                    .iter()
                    .find(|(r, _)| pt_in_rect(*r, x, y))
                    .map(|(_, a)| *a);
                if let Some(act) = hit {
                    match act {
                        ActionId::ToggleStartMenu => st.opts.start_menu = !st.opts.start_menu,
                        ActionId::ToggleDesktop => {
                            st.opts.desktop_shortcut = !st.opts.desktop_shortcut;
                        }
                        ActionId::ToggleStartup => {
                            st.opts.run_on_startup = !st.opts.run_on_startup;
                        }
                        ActionId::ToggleLaunch => st.opts.launch_after = !st.opts.launch_after,
                        ActionId::ToggleRemoveData => {
                            st.opts.remove_user_data = !st.opts.remove_user_data;
                        }
                        ActionId::CancelButton => return true,
                        ActionId::PrimaryButton => {
                            if st.phase != WizardPhase::Ready {
                                return true;
                            }
                            let res = if st.opts.uninstall_mode {
                                run_uninstall(&st.opts)
                            } else {
                                run_install(&st.opts)
                            };
                            match res {
                                Ok(msg) => {
                                    st.phase = WizardPhase::Done;
                                    st.status_msg = msg;
                                }
                                Err(err) => {
                                    st.phase = WizardPhase::Failed;
                                    st.status_msg = err;
                                }
                            }
                        }
                    }
                }
                false
            });
            if quit {
                unsafe { PostQuitMessage(0) };
            } else {
                unsafe {
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_CLOSE | WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn parse_args() -> InstallOptions {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut opts = InstallOptions {
        install_dir: default_install_dir(),
        uninstall_mode: false,
        silent: false,
        start_menu: true,
        desktop_shortcut: true,
        run_on_startup: true,
        launch_after: true,
        remove_user_data: false,
        skip_system_hooks: false,
    };
    for a in &args {
        let lower = a.to_ascii_lowercase();
        if lower == "--uninstall" || lower == "/u" {
            opts.uninstall_mode = true;
        } else if lower == "/s" || lower == "--silent" || lower == "/quiet" {
            opts.silent = true;
            opts.launch_after = false;
        } else if lower == "--no-hooks" {
            opts.skip_system_hooks = true;
        } else if let Some(rest) = a.strip_prefix("--dir=") {
            opts.install_dir = PathBuf::from(rest.trim_matches('"'));
        }
    }
    opts
}

fn main() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let opts = parse_args();
    if opts.silent {
        let res = if opts.uninstall_mode {
            run_uninstall(&opts)
        } else {
            run_install(&opts)
        };
        unsafe { CoUninitialize() };
        if res.is_err() {
            std::process::exit(1);
        }
        return;
    }

    let dpi_scale = unsafe {
        let hdc = GetDC(None);
        let logpx = GetDeviceCaps(hdc, LOGPIXELSX);
        ReleaseDC(None, hdc);
        (logpx.max(96) as f32) / 96.0
    };
    let win_w = (540.0 * dpi_scale).round() as i32;
    let win_h = (472.0 * dpi_scale).round() as i32;
    let (x, y) = unsafe {
        let sx = GetSystemMetrics(SM_CXSCREEN);
        let sy = GetSystemMetrics(SM_CYSCREEN);
        (((sx - win_w) / 2).max(20), ((sy - win_h) / 2).max(20))
    };

    let title = if opts.uninstall_mode {
        format!("Uninstall {APP_NAME}")
    } else {
        format!("{APP_NAME} v{APP_VERSION} Setup")
    };

    STATE.with(|cell| {
        *cell.borrow_mut() = Some(WizardState {
            opts,
            phase: WizardPhase::Ready,
            status_msg: String::new(),
            logo_icon: create_icon((52.0 * dpi_scale).round() as i32),
            hits: Vec::new(),
            hovered: None,
        });
    });

    let class_w = to_wide(CLASS_NAME);
    let title_w = to_wide(&title);
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hIcon: create_icon(32).unwrap_or_default(),
            hIconSm: create_icon(16).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        if let Ok(hwnd) = CreateWindowExW(
            Default::default(),
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            x,
            y,
            win_w,
            win_h,
            None,
            None,
            hinst,
            None,
        ) {
            let dark: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&dark as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            );
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = InvalidateRect(hwnd, None, false);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        CoUninitialize();
    }
}
