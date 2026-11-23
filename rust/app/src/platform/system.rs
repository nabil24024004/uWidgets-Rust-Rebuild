use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use windows::Win32::Foundation::{FILETIME, HWND, LPARAM, MAX_PATH, SYSTEMTIME};
use windows::Win32::Graphics::Dwm::{
    DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DWM_SYSTEMBACKDROP_TYPE,
    DWM_WINDOW_CORNER_PREFERENCE, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND, DwmEnableBlurBehindWindow,
    DwmExtendFrameIntoClientArea, DwmGetColorizationColor, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_BINARY, REG_DWORD,
    REG_SZ, RegCloseKey, RegDeleteValueW, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{GetCurrentProcess, GetSystemTimes};
use windows::Win32::System::Time::{
    DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation,
    SystemTimeToTzSpecificLocalTimeEx, TIME_ZONE_INFORMATION,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, GWL_EXSTYLE, GetWindowLongPtrW, HICON, LR_DEFAULTCOLOR,
    SPI_GETDESKWALLPAPER, SW_SHOWNORMAL, SetWindowLongPtrW, SystemParametersInfoW,
    WS_EX_TOOLWINDOW,
};
use windows_core::{PCWSTR, PWSTR};

use crate::monitor_rs;

const APP_ICO_BYTES: &[u8] = include_bytes!("../../../../assets/uWidgets.ico");

pub fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn from_wide_slice(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

pub fn create_app_icon(desired_size: i32) -> Option<HICON> {
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
        let diff = (w - desired_size).abs();
        if offset + bytes_len <= APP_ICO_BYTES.len()
            && best.is_none_or(|(_, _, best_diff)| diff < best_diff)
        {
            best = Some((offset, bytes_len, diff));
        }
    }
    let (offset, len, _) = best?;
    let slice = &APP_ICO_BYTES[offset..offset + len];
    unsafe {
        CreateIconFromResourceEx(
            slice,
            true,
            0x0003_0000,
            desired_size,
            desired_size,
            LR_DEFAULTCOLOR,
        )
        .ok()
    }
}

pub fn get_wallpaper_path() -> String {
    let mut buf = [0u16; MAX_PATH as usize];
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr().cast()),
            Default::default(),
        );
    }
    from_wide_slice(&buf)
}

pub fn remove_window_from_alt_tab(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex = ex | (WS_EX_TOOLWINDOW.0 as isize);
        if new_ex != ex {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
        }
    }
}

pub fn apply_window_dwm(hwnd: HWND, is_dark: bool, uses_acrylic: bool, use_native_frame: bool) {
    unsafe {
        let dark: i32 = if is_dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        let corner: DWM_WINDOW_CORNER_PREFERENCE = if use_native_frame {
            DWMWCP_ROUND
        } else {
            DWMWCP_DONOTROUND
        };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        // Extend frame across entire client area so premultiplied DirectComposition alpha composites cleanly.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        // Keep DWM system backdrop disabled on widget windows because Win11 DWMSBT_TRANSIENTWINDOW
        // automatically collapses to solid opaque gray whenever a desktop widget loses focus.
        // Instead, our DirectComposition glass engine renders always-on translucent Acrylic & Liquid Glass
        // with true OpacityLevel control while preserving native DWM rounded corners & drop shadow.
        let _ = uses_acrylic;
        let backdrop_val = DWM_SYSTEMBACKDROP_TYPE(1); // DWMSBT_NONE
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop_val as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        );

        // Clear any legacy blur region if not native frame
        if !use_native_frame {
            let rgn = CreateRectRgn(0, 0, -1, -1);
            let bb = DWM_BLURBEHIND {
                dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                fEnable: false.into(),
                hRgnBlur: rgn,
                fTransitionOnMaximized: false.into(),
            };
            let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
            let _ = DeleteObject(rgn);
        }
    }
}

pub fn get_windows_dark_mode() -> bool {
    let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value_name = to_wide("AppsUseLightTheme");
    let mut hkey = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let mut val: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            let mut kind = REG_DWORD;
            let ok = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut kind),
                Some((&mut val as *mut u32).cast()),
                Some(&mut size),
            )
            .is_ok();
            let _ = RegCloseKey(hkey);
            if ok {
                return val == 0;
            }
        }
    }
    true
}

pub fn get_windows_accent_color() -> (f32, f32, f32) {
    unsafe {
        let mut color: u32 = 0;
        let mut opaque = windows::Win32::Foundation::BOOL(0);
        if DwmGetColorizationColor(&mut color, &mut opaque).is_ok() {
            let r = ((color >> 16) & 0xFF) as f32 / 255.0;
            let g = ((color >> 8) & 0xFF) as f32 / 255.0;
            let b = (color & 0xFF) as f32 / 255.0;
            return (r, g, b);
        }
    }
    (0.20, 0.46, 0.80) // #3376CD fallback matching AppearanceViewModel
}

pub fn set_run_on_startup(enable: bool) -> bool {
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
        .is_err()
        {
            return false;
        }
        let ok = if enable {
            let exe = std::env::current_exe()
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Always quote path in Windows Run registry value so paths with spaces launch cleanly!
            let quoted = format!("\"{exe}\"");
            let wide = to_wide(&quoted);
            let bytes = std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2);
            RegSetValueExW(hkey, PCWSTR(val_name.as_ptr()), 0, REG_SZ, Some(bytes)).is_ok()
        } else {
            RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr())).is_ok()
        };
        let _ = RegCloseKey(hkey);
        ok
    }
}

pub fn open_url(url: &str) {
    let op = to_wide("open");
    let target = to_wide(url);
    unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(op.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

pub fn get_process_memory_mb() -> (f64, f64) {
    unsafe {
        let mut pmc = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb).is_ok() {
            let ws = pmc.WorkingSetSize as f64 / (1024.0 * 1024.0);
            let priv_mb = pmc.PagefileUsage as f64 / (1024.0 * 1024.0);
            return (ws, priv_mb);
        }
    }
    (0.0, 0.0)
}

pub fn get_clipboard_text(hwnd: HWND) -> Option<String> {
    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return None;
        }
        let h = GetClipboardData(13 /* CF_UNICODETEXT */).ok();
        let result = h.and_then(|handle| {
            let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0)) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 && len < 65536 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
            Some(s)
        });
        let _ = CloseClipboard();
        result
    }
}

#[allow(dead_code)]
pub fn set_clipboard_text(hwnd: HWND, text: &str) -> bool {
    let wide = to_wide(text);
    let byte_len = wide.len() * 2;
    unsafe {
        if OpenClipboard(hwnd).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem) as *mut u16;
            if !ptr.is_null() {
                ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(hmem);
                SetClipboardData(13, windows::Win32::Foundation::HANDLE(hmem.0)).is_ok()
            } else {
                false
            }
        } else {
            false
        };
        let _ = CloseClipboard();
        ok
    }
}

// --- Windows Registry Timezone Support (exact match with C# TimeZoneInfo) ---

#[derive(Debug, Clone)]
pub struct WinTimeZone {
    pub id: String,
    pub display_name: String,
    pub bias: i32,
    pub std_bias: i32,
    pub dst_bias: i32,
    pub std_date: SYSTEMTIME,
    pub dst_date: SYSTEMTIME,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RegTziFormat {
    bias: i32,
    standard_bias: i32,
    daylight_bias: i32,
    standard_date: SYSTEMTIME,
    daylight_date: SYSTEMTIME,
}

fn read_reg_string(hkey: HKEY, name: &str) -> Option<String> {
    let wname = to_wide(name);
    let mut size: u32 = 0;
    let mut kind = REG_SZ;
    unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
        .ok()
        .ok()?;
        if size == 0 {
            return Some(String::new());
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        RegQueryValueExW(
            hkey,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
        Some(from_wide_slice(&buf))
    }
}

#[allow(dead_code)]
pub fn get_local_timezone_id() -> String {
    unsafe {
        let mut dtzi = DYNAMIC_TIME_ZONE_INFORMATION::default();
        GetDynamicTimeZoneInformation(&mut dtzi);
        let key = from_wide_slice(&dtzi.TimeZoneKeyName);
        if !key.is_empty() {
            return key;
        }
        from_wide_slice(&dtzi.StandardName)
    }
}

pub fn load_system_timezones() -> Vec<WinTimeZone> {
    let root_path = to_wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Time Zones");
    let mut root_key = HKEY::default();
    let mut zones = Vec::new();
    unsafe {
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(root_path.as_ptr()),
            0,
            KEY_READ,
            &mut root_key,
        )
        .is_err()
        {
            return zones;
        }
        let mut idx = 0u32;
        loop {
            let mut name_buf = [0u16; 128];
            let mut name_len = name_buf.len() as u32;
            if RegEnumKeyExW(
                root_key,
                idx,
                PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                PWSTR::null(),
                None,
                None,
            )
            .is_err()
            {
                break;
            }
            idx += 1;
            let id = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let sub_w = to_wide(&id);
            let mut sub_key = HKEY::default();
            if RegOpenKeyExW(root_key, PCWSTR(sub_w.as_ptr()), 0, KEY_READ, &mut sub_key).is_ok() {
                let display_name =
                    read_reg_string(sub_key, "Display").unwrap_or_else(|| id.clone());
                let tzi_name = to_wide("TZI");
                let mut tzi = RegTziFormat {
                    bias: 0,
                    standard_bias: 0,
                    daylight_bias: 0,
                    standard_date: SYSTEMTIME::default(),
                    daylight_date: SYSTEMTIME::default(),
                };
                let mut size = std::mem::size_of::<RegTziFormat>() as u32;
                let mut kind = REG_BINARY;
                if RegQueryValueExW(
                    sub_key,
                    PCWSTR(tzi_name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some((&mut tzi as *mut RegTziFormat).cast()),
                    Some(&mut size),
                )
                .is_ok()
                {
                    zones.push(WinTimeZone {
                        id,
                        display_name,
                        bias: tzi.bias,
                        std_bias: tzi.standard_bias,
                        dst_bias: tzi.daylight_bias,
                        std_date: tzi.standard_date,
                        dst_date: tzi.daylight_date,
                    });
                }
                let _ = RegCloseKey(sub_key);
            }
        }
        let _ = RegCloseKey(root_key);
    }
    zones.sort_by(|a, b| {
        (-a.bias)
            .cmp(&(-b.bias))
            .then(a.display_name.cmp(&b.display_name))
    });
    zones
}

pub fn now_in_timezone(tz: Option<&WinTimeZone>) -> NaiveDateTime {
    let Some(tz) = tz else {
        return Local::now().naive_local();
    };
    let utc: DateTime<Utc> = Utc::now();
    let st_utc = SYSTEMTIME {
        wYear: utc.year() as u16,
        wMonth: utc.month() as u16,
        wDayOfWeek: utc.weekday().num_days_from_sunday() as u16,
        wDay: utc.day() as u16,
        wHour: utc.hour() as u16,
        wMinute: utc.minute() as u16,
        wSecond: utc.second() as u16,
        wMilliseconds: (utc.timestamp_subsec_millis() as u16).min(999),
    };
    let mut dtzi = DYNAMIC_TIME_ZONE_INFORMATION {
        Bias: tz.bias,
        StandardBias: tz.std_bias,
        DaylightBias: tz.dst_bias,
        StandardDate: tz.std_date,
        DaylightDate: tz.dst_date,
        ..Default::default()
    };
    let id_w = to_wide(&tz.id);
    for (dst, &src) in dtzi.TimeZoneKeyName.iter_mut().zip(id_w.iter()) {
        *dst = src;
    }
    let mut st_local = SYSTEMTIME::default();
    let ok =
        unsafe { SystemTimeToTzSpecificLocalTimeEx(Some(&dtzi), &st_utc, &mut st_local).is_ok() };
    if ok
        && let Some(date) = NaiveDate::from_ymd_opt(
            i32::from(st_local.wYear),
            u32::from(st_local.wMonth),
            u32::from(st_local.wDay),
        )
        && let Some(time) = NaiveTime::from_hms_milli_opt(
            u32::from(st_local.wHour),
            u32::from(st_local.wMinute),
            u32::from(st_local.wSecond),
            u32::from(st_local.wMilliseconds),
        )
    {
        return NaiveDateTime::new(date, time);
    }
    // Fallback via fixed bias if dynamic conversion fails
    let _ = TIME_ZONE_INFORMATION::default();
    utc.naive_utc() - chrono::Duration::minutes(i64::from(tz.bias))
}

// --- Native Windows Telemetry Collector (zero WMI!) ---

pub struct MonitorCollector {
    initialized: bool,
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    prev_net_bytes: u64,
    prev_net_instant: std::time::Instant,
    pdh_query: isize,
    pdh_disk_counter: isize,
    pub cpu: f64,
    pub ram: f64,
    pub disk: f64,
    pub network: f64,
    pub battery: Option<f64>,
}

fn ft_to_u64(ft: FILETIME) -> u64 {
    (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
}

impl MonitorCollector {
    pub fn new() -> Self {
        Self {
            initialized: false,
            prev_idle: 0,
            prev_kernel: 0,
            prev_user: 0,
            prev_net_bytes: 0,
            prev_net_instant: std::time::Instant::now(),
            pdh_query: 0,
            pdh_disk_counter: 0,
            cpu: 0.0,
            ram: 0.0,
            disk: 0.0,
            network: 0.0,
            battery: None,
        }
    }

    fn ensure_init(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        unsafe {
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                self.prev_idle = ft_to_u64(idle);
                self.prev_kernel = ft_to_u64(kernel);
                self.prev_user = ft_to_u64(user);
            }
            let mut q: isize = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) == 0 {
                let path = to_wide("\\LogicalDisk(_Total)\\% Disk Time");
                let mut c: isize = 0;
                if PdhAddEnglishCounterW(q, PCWSTR(path.as_ptr()), 0, &mut c) == 0 {
                    let _ = PdhCollectQueryData(q);
                    self.pdh_query = q;
                    self.pdh_disk_counter = c;
                } else {
                    let _ = PdhCloseQuery(q);
                }
            }
        }
        self.prev_net_bytes = Self::sample_net_bytes().0;
        self.prev_net_instant = std::time::Instant::now();
    }

    fn sample_net_bytes() -> (u64, u64) {
        let mut total_bytes = 0u64;
        let mut max_speed = 0u64;
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = ptr::null_mut();
            if GetIfTable2(&mut table).is_ok() && !table.is_null() {
                let count = (*table).NumEntries as usize;
                let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), count);
                for row in rows {
                    let r: &MIB_IF_ROW2 = row;
                    // Skip software loopback (IfType == 24) and down adapters
                    if r.Type == 24 || r.OperStatus != IfOperStatusUp {
                        continue;
                    }
                    total_bytes = total_bytes
                        .saturating_add(r.InOctets)
                        .saturating_add(r.OutOctets);
                    if r.TransmitLinkSpeed > max_speed && r.TransmitLinkSpeed < u64::MAX / 2 {
                        max_speed = r.TransmitLinkSpeed;
                    }
                }
                FreeMibTable(table.cast());
            }
        }
        (total_bytes, max_speed)
    }

    pub fn sample(&mut self) {
        self.ensure_init();
        // CPU
        unsafe {
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                let i = ft_to_u64(idle);
                let k = ft_to_u64(kernel);
                let u = ft_to_u64(user);
                let di = i.saturating_sub(self.prev_idle);
                let dk = k.saturating_sub(self.prev_kernel);
                let du = u.saturating_sub(self.prev_user);
                let total = dk.saturating_add(du);
                if total > 0 {
                    self.cpu = (total.saturating_sub(di) as f64 / total as f64).clamp(0.0, 1.0);
                }
                self.prev_idle = i;
                self.prev_kernel = k;
                self.prev_user = u;
            }
        }
        // RAM (delegating ratio math to gated monitor_rs::port::compute_ram_ratio)
        unsafe {
            let mut ms = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            if GlobalMemoryStatusEx(&mut ms).is_ok() {
                let ratio = monitor_rs::port::compute_ram_ratio(
                    ms.ullTotalPhys as f64,
                    ms.ullAvailPhys as f64,
                )
                .unwrap_or(0.0);
                self.ram = ratio.clamp(0.0, 1.0);
            }
        }
        // Disk via PDH
        if self.pdh_query != 0 && self.pdh_disk_counter != 0 {
            unsafe {
                if PdhCollectQueryData(self.pdh_query) == 0 {
                    let mut val = PDH_FMT_COUNTERVALUE::default();
                    if PdhGetFormattedCounterValue(
                        self.pdh_disk_counter,
                        PDH_FMT_DOUBLE,
                        None,
                        &mut val,
                    ) == 0
                    {
                        self.disk = (val.Anonymous.doubleValue / 100.0).clamp(0.0, 1.0);
                    }
                }
            }
        }
        // Network (delegating ratio math to gated monitor_rs::port::compute_network_ratio)
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.prev_net_instant).as_secs_f64();
        let (bytes, speed) = Self::sample_net_bytes();
        if elapsed >= 0.2 && speed > 0 {
            let delta = bytes.saturating_sub(self.prev_net_bytes) as f64 / elapsed;
            let ratio =
                monitor_rs::port::compute_network_ratio(delta, 0.0, speed as f64).unwrap_or(0.0);
            self.network = ratio.clamp(0.0, 1.0);
            self.prev_net_bytes = bytes;
            self.prev_net_instant = now;
        }
        // Battery
        unsafe {
            let mut sps = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut sps).is_ok() && sps.BatteryLifePercent <= 100 {
                self.battery = Some(f64::from(sps.BatteryLifePercent) / 100.0);
            } else {
                self.battery = None;
            }
        }
    }

    pub fn value(&self, metric: i32) -> f64 {
        match metric {
            0 => self.cpu,
            1 => self.ram,
            2 => self.disk,
            3 => self.network,
            4 => self.battery.unwrap_or(0.0),
            _ => self.cpu,
        }
    }
}

impl Drop for MonitorCollector {
    fn drop(&mut self) {
        if self.pdh_query != 0 {
            unsafe {
                let _ = PdhCloseQuery(self.pdh_query);
            }
        }
    }
}

pub fn lparam_point(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}
