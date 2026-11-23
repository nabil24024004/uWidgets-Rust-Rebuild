use std::cell::RefCell;
use std::rc::Rc;

use chrono::Local;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_BACK, VK_CONTROL, VK_ESCAPE, VK_RETURN,
    VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyMenu, GWLP_USERDATA, GetClientRect, GetCursorPos, GetWindowLongPtrW,
    GetWindowRect, HWND_BOTTOM, IDC_ARROW, IDC_SIZENWSE, LoadCursorW, MF_POPUP, MF_SEPARATOR,
    MF_STRING, RegisterClassW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetCursor, SetWindowLongPtrW, SetWindowPos, ShowWindow, TPM_LEFTALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu, WM_CHAR, WM_CONTEXTMENU, WM_KEYDOWN,
    WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
    WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WNDCLASSW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
    WS_POPUP, WS_VISIBLE,
};
use windows_core::PCWSTR;

use crate::core_rs;
use crate::i18n::tr;
use crate::models::{NoteModel, ReminderModel, RemindersListModel};
use crate::notes_reminders_rs;
use crate::platform::graphics::{WindowSurface, draw_arc, rect, rgba};
use crate::platform::liquid_glass::WidgetGlassCache;
use crate::platform::system::{
    apply_window_dwm, get_clipboard_text, lparam_point, remove_window_from_alt_tab, to_wide,
};
use crate::ui::{AppCommand, AppState, enqueue_cmd, get_global_app};
use crate::widgets::{
    EditFieldId, InlineEditState, WidgetHitAction, WidgetHitRegion, render_widget_card,
    subtype_has_settings,
};

const CLASS_NAME: &str = "uWidgetsRsWidget";
const RESIZE_GRIP_DIP: f32 = 16.0;

const MENU_EDIT: u32 = 2001;
const MENU_SIZE_SMALL: u32 = 2010;
const MENU_SIZE_MEDIUM: u32 = 2011;
const MENU_SIZE_LARGE: u32 = 2012;
const MENU_SIZE_XLARGE: u32 = 2013;
const MENU_REMOVE: u32 = 2020;
const MENU_SETTINGS: u32 = 2030;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragMode {
    None,
    Move,
    Resize,
}

pub struct WidgetWindow {
    pub hwnd: HWND,
    pub surface: WindowSurface,
    pub glass_cache: WidgetGlassCache,
    pub hits: Vec<WidgetHitRegion>,
    pub inline_edit: Option<InlineEditState>,
    drag_mode: DragMode,
    drag_start_screen: POINT,
    drag_start_rect: RECT,
    hover_resize: bool,
}

impl WidgetWindow {
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
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
    }
}

pub fn spawn_widget_window(app: &Rc<RefCell<AppState>>, id: u64) {
    let (layout, theme, gfx) = {
        let st = app.borrow();
        let Some(l) = st.find_layout(id).cloned() else {
            return;
        };
        (l, st.resolved_theme(), Rc::clone(&st.gfx))
    };

    let class_w = to_wide(CLASS_NAME);
    let title_w = to_wide(&format!("{}_{}", layout.widget_type, layout.sub_type));

    let hwnd = unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_POPUP | WS_VISIBLE,
            layout.x,
            layout.y,
            layout.width.max(40),
            layout.height.max(40),
            None,
            None,
            hinst,
            None,
        )
    };
    let Ok(hwnd) = hwnd else { return };

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, id as isize);
    }
    remove_window_from_alt_tab(hwnd);
    apply_window_dwm(
        hwnd,
        theme.is_dark,
        theme.uses_acrylic,
        theme.use_native_frame,
    );

    // `layout.width` and `layout.height` are stored in 96-DPI logical DIPs (matching Avalonia),
    // while `layout.x` and `layout.y` are physical screen pixels.
    let dpi = unsafe { (GetDpiForWindow(hwnd) as f32).max(96.0) };
    let scale = dpi / 96.0;
    let phys_w = ((layout.width.max(36) as f32) * scale).round() as i32;
    let phys_h = ((layout.height.max(36) as f32) * scale).round() as i32;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            layout.x,
            layout.y,
            phys_w,
            phys_h,
            SWP_NOACTIVATE,
        );
    }

    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rc);
    }
    let cw = (rc.right - rc.left).max(1) as u32;
    let ch = (rc.bottom - rc.top).max(1) as u32;

    if let Ok(surface) = WindowSurface::new(gfx, hwnd, cw, ch, dpi) {
        app.borrow_mut().widget_windows.insert(
            id,
            WidgetWindow {
                hwnd,
                surface,
                glass_cache: WidgetGlassCache::new(),
                hits: Vec::new(),
                inline_edit: None,
                drag_mode: DragMode::None,
                drag_start_screen: POINT::default(),
                drag_start_rect: RECT::default(),
                hover_resize: false,
            },
        );
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                HWND_BOTTOM,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let _ = InvalidateRect(hwnd, None, false);
        }
    }
}

pub fn resize_widget_hwnd(hwnd: HWND, logical_width: i32, logical_height: i32) {
    unsafe {
        let dpi = (GetDpiForWindow(hwnd) as f32).max(96.0);
        let scale = dpi / 96.0;
        let pw = ((logical_width.max(36) as f32) * scale).round() as i32;
        let ph = ((logical_height.max(36) as f32) * scale).round() as i32;
        let _ = SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, pw, ph, SWP_NOMOVE | SWP_NOACTIVATE);
    }
}

fn widget_id_of(hwnd: HWND) -> u64 {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u64 }
}

fn commit_inline_edit(app: &Rc<RefCell<AppState>>, id: u64) {
    let mut st = app.borrow_mut();
    let edit = st
        .widget_windows
        .get_mut(&id)
        .and_then(|w| w.inline_edit.take());
    let Some(edit) = edit else { return };

    let mut changed = false;
    if let Some(layout) = st.find_layout_mut(id) {
        match edit.field {
            EditFieldId::NoteTitle => {
                let mut m: NoteModel = layout.parse_settings();
                m.title = Some(edit.buffer);
                m.updated = Some(Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
                layout.set_settings(&m);
                changed = true;
            }
            EditFieldId::NoteContent => {
                let mut m: NoteModel = layout.parse_settings();
                m.content = Some(edit.buffer);
                m.updated = Some(Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
                layout.set_settings(&m);
                changed = true;
            }
            EditFieldId::ReminderListName => {
                let mut m: RemindersListModel = layout.parse_settings();
                m.list_name = Some(edit.buffer);
                layout.set_settings(&m);
                changed = true;
            }
            EditFieldId::ReminderItem(idx) => {
                let mut m: RemindersListModel = layout.parse_settings();
                if idx < m.reminders.len() {
                    if notes_reminders_rs::port::should_remove_reminder_on_edit(edit.buffer.clone())
                        .unwrap_or(false)
                    {
                        m.reminders.remove(idx);
                    } else {
                        m.reminders[idx].title = edit.buffer;
                    }
                    layout.set_settings(&m);
                    changed = true;
                }
            }
            EditFieldId::ReminderNew => {
                if notes_reminders_rs::port::should_add_reminder(edit.buffer.clone())
                    .unwrap_or(false)
                {
                    let mut m: RemindersListModel = layout.parse_settings();
                    m.reminders.push(ReminderModel {
                        completed: false,
                        title: edit.buffer,
                    });
                    layout.set_settings(&m);
                    changed = true;
                }
            }
        }
    }
    if changed {
        st.save_layouts();
    }
    if let Some(w) = st.widget_windows.get(&id) {
        w.invalidate();
    }
}

fn show_context_menu(hwnd: HWND, id: u64) {
    let info = {
        let Some(app) = get_global_app() else { return };
        let st = app.borrow();
        let Some(l) = st.find_layout(id) else { return };
        (
            st.settings.region.language.clone(),
            subtype_has_settings(&l.widget_type, &l.sub_type),
        )
    };
    let (lang, has_settings) = info;

    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        if has_settings {
            let s_edit = to_wide(tr(&lang, "Widget_Edit"));
            let _ = AppendMenuW(menu, MF_STRING, MENU_EDIT as usize, PCWSTR(s_edit.as_ptr()));
        }

        if let Ok(size_menu) = CreatePopupMenu() {
            let s_sm = to_wide(tr(&lang, "Widget_Size_Small"));
            let s_md = to_wide(tr(&lang, "Widget_Size_Medium"));
            let s_lg = to_wide(tr(&lang, "Widget_Size_Large"));
            let s_xl = to_wide(tr(&lang, "Widget_Size_ExtraLarge"));
            let _ = AppendMenuW(
                size_menu,
                MF_STRING,
                MENU_SIZE_SMALL as usize,
                PCWSTR(s_sm.as_ptr()),
            );
            let _ = AppendMenuW(
                size_menu,
                MF_STRING,
                MENU_SIZE_MEDIUM as usize,
                PCWSTR(s_md.as_ptr()),
            );
            let _ = AppendMenuW(
                size_menu,
                MF_STRING,
                MENU_SIZE_LARGE as usize,
                PCWSTR(s_lg.as_ptr()),
            );
            let _ = AppendMenuW(
                size_menu,
                MF_STRING,
                MENU_SIZE_XLARGE as usize,
                PCWSTR(s_xl.as_ptr()),
            );
            let s_size = to_wide(tr(&lang, "Widget_Size"));
            let _ = AppendMenuW(
                menu,
                MF_POPUP,
                size_menu.0 as usize,
                PCWSTR(s_size.as_ptr()),
            );
        }

        let s_remove = to_wide(tr(&lang, "Widget_Remove"));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_REMOVE as usize,
            PCWSTR(s_remove.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let s_open = to_wide(tr(&lang, "Widget_OpenSettings"));
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_SETTINGS as usize,
            PCWSTR(s_open.as_ptr()),
        );

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let chosen = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        )
        .0 as u32;
        let _ = DestroyMenu(menu);

        match chosen {
            MENU_EDIT => enqueue_cmd(AppCommand::OpenEditWidget(id)),
            MENU_SIZE_SMALL => enqueue_cmd(AppCommand::ResizeWidgetPreset {
                id,
                preset_index: 0,
            }),
            MENU_SIZE_MEDIUM => enqueue_cmd(AppCommand::ResizeWidgetPreset {
                id,
                preset_index: 1,
            }),
            MENU_SIZE_LARGE => enqueue_cmd(AppCommand::ResizeWidgetPreset {
                id,
                preset_index: 2,
            }),
            MENU_SIZE_XLARGE => enqueue_cmd(AppCommand::ResizeWidgetPreset {
                id,
                preset_index: 3,
            }),
            MENU_REMOVE => enqueue_cmd(AppCommand::RemoveWidget(id)),
            MENU_SETTINGS => enqueue_cmd(AppCommand::OpenSettings),
            _ => {}
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let id = widget_id_of(hwnd);
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            if id != 0
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                let theme = st.resolved_theme();
                let dims = st.settings.dimensions.clone();
                let lg = st.settings.liquid_glass.clone();
                let weather_cfg = st.settings.weather.clone();
                let font = st.settings.theme.font_family.clone();
                let lang = st.settings.region.language.clone();
                let lock_size = st.settings.layout.lock_size;
                let layout = st.find_layout(id).cloned();

                if let Some(layout) = layout {
                    let AppState {
                        gfx,
                        monitor,
                        weather,
                        timezones,
                        widget_windows,
                        ..
                    } = &mut *st;
                    if let Some(win) = widget_windows.get_mut(&id) {
                        let w = win.surface.width as f32 * 96.0 / win.surface.dpi;
                        let h = win.surface.height as f32 * 96.0 / win.surface.dpi;
                        // Render always-on translucent Acrylic (Windows theme) or SDF Liquid Glass (Apple theme)
                        // whenever OpacityLevel < 1.0!
                        let glass_bmp = if theme.uses_acrylic {
                            let mut wrc = RECT::default();
                            unsafe {
                                let _ = GetWindowRect(hwnd, &mut wrc);
                            }
                            let blur_r = lg.blur_radius.clamp(0, 32) as usize;
                            gfx.backdrop.with_backdrop(blur_r, |bd| {
                                win.glass_cache.get_or_build(
                                    &win.surface.dc,
                                    bd,
                                    wrc.left,
                                    wrc.top,
                                    win.surface.width,
                                    win.surface.height,
                                    win.surface.dpi,
                                    theme.radius,
                                    theme,
                                    &lg,
                                )
                            })
                        } else {
                            win.glass_cache.clear();
                            None
                        };
                        win.hits.clear();
                        win.surface.begin();

                        render_widget_card(
                            gfx,
                            &win.surface.dc,
                            &layout,
                            rect(0.0, 0.0, w, h),
                            theme,
                            &dims,
                            &font,
                            &lang,
                            monitor,
                            weather,
                            &weather_cfg,
                            timezones,
                            win.inline_edit.as_ref(),
                            &mut win.hits,
                            false,
                            glass_bmp.as_ref(),
                        );

                        // Subtle bottom-right resize grip hint when unlocked and hovered
                        if !lock_size && win.hover_resize {
                            draw_arc(
                                gfx,
                                &win.surface.dc,
                                w - 12.0,
                                h - 12.0,
                                6.0,
                                95.0,
                                80.0,
                                2.0,
                                rgba(
                                    theme.accent_effective.0,
                                    theme.accent_effective.1,
                                    theme.accent_effective.2,
                                    0.85,
                                ),
                            );
                        }

                        let _ = win.surface.end();
                    }
                }
            }
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if id != 0
                && let Some(app) = get_global_app()
            {
                let mut st = app.borrow_mut();
                if let Some(win) = st.widget_windows.get_mut(&id) {
                    let mut rc = RECT::default();
                    let dpi = unsafe {
                        let _ = GetClientRect(hwnd, &mut rc);
                        (GetDpiForWindow(hwnd) as f32).max(96.0)
                    };
                    let cw = (rc.right - rc.left).max(1) as u32;
                    let ch = (rc.bottom - rc.top).max(1) as u32;
                    let _ = win.surface.resize(cw, ch, dpi);
                }
            }
            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            if id != 0
                && let Some(app) = get_global_app()
            {
                let st = app.borrow();
                let show_resize = !st.settings.layout.lock_size
                    && st.widget_windows.get(&id).is_some_and(|w| w.hover_resize);
                if show_resize {
                    unsafe {
                        if let Ok(cur) = LoadCursorW(None, IDC_SIZENWSE) {
                            SetCursor(cur);
                        }
                    }
                    return LRESULT(1);
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_LBUTTONDOWN => {
            let (px, py) = lparam_point(lparam);
            if id != 0
                && let Some(app) = get_global_app()
            {
                // 1. Check if user clicked a Reminders checkbox (single-click toggles immediately!)
                let toggle_idx = {
                    let st = app.borrow();
                    st.widget_windows.get(&id).and_then(|win| {
                        let dx = px as f32 * 96.0 / win.surface.dpi;
                        let dy = py as f32 * 96.0 / win.surface.dpi;
                        win.hits.iter().find_map(|hr| {
                            if dx >= hr.rect.left
                                && dx <= hr.rect.right
                                && dy >= hr.rect.top
                                && dy <= hr.rect.bottom
                                && let WidgetHitAction::ToggleReminder(i) = hr.action
                            {
                                return Some(i);
                            }
                            None
                        })
                    })
                };

                if let Some(idx) = toggle_idx {
                    let mut st = app.borrow_mut();
                    if let Some(layout) = st.find_layout_mut(id) {
                        let mut m: RemindersListModel = layout.parse_settings();
                        if let Some(item) = m.reminders.get_mut(idx) {
                            item.completed = !item.completed;
                            layout.set_settings(&m);
                        }
                    }
                    st.save_layouts();
                    if let Some(win) = st.widget_windows.get(&id) {
                        win.invalidate();
                    }
                    return LRESULT(0);
                }

                // Commit any open inline edit if clicking elsewhere
                commit_inline_edit(&app, id);

                // 2. Determine Move vs Resize drag
                let mut cur_screen = POINT::default();
                let mut wrc = RECT::default();
                unsafe {
                    let _ = GetCursorPos(&mut cur_screen);
                    let _ = GetWindowRect(hwnd, &mut wrc);
                }
                let start_capture = {
                    let mut st = app.borrow_mut();
                    let lock_pos = st.settings.layout.lock_position;
                    let lock_size = st.settings.layout.lock_size;
                    if let Some(win) = st.widget_windows.get_mut(&id) {
                        let dx = px as f32 * 96.0 / win.surface.dpi;
                        let dy = py as f32 * 96.0 / win.surface.dpi;
                        let w = win.surface.width as f32 * 96.0 / win.surface.dpi;
                        let h = win.surface.height as f32 * 96.0 / win.surface.dpi;
                        let in_grip = dx >= w - RESIZE_GRIP_DIP && dy >= h - RESIZE_GRIP_DIP;
                        if in_grip && !lock_size {
                            win.drag_mode = DragMode::Resize;
                            win.drag_start_screen = cur_screen;
                            win.drag_start_rect = wrc;
                            true
                        } else if !lock_pos {
                            win.drag_mode = DragMode::Move;
                            win.drag_start_screen = cur_screen;
                            win.drag_start_rect = wrc;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if start_capture {
                    unsafe { SetCapture(hwnd) };
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            let (px, py) = lparam_point(lparam);
            if id != 0
                && let Some(app) = get_global_app()
            {
                let field_opt = {
                    let st = app.borrow();
                    st.widget_windows.get(&id).and_then(|win| {
                        let dx = px as f32 * 96.0 / win.surface.dpi;
                        let dy = py as f32 * 96.0 / win.surface.dpi;
                        win.hits.iter().find_map(|hr| {
                            if dx >= hr.rect.left
                                && dx <= hr.rect.right
                                && dy >= hr.rect.top
                                && dy <= hr.rect.bottom
                                && let WidgetHitAction::BeginEdit(ref f) = hr.action
                            {
                                return Some(f.clone());
                            }
                            None
                        })
                    })
                };

                if let Some(field) = field_opt {
                    let mut st = app.borrow_mut();
                    let initial = if let Some(layout) = st.find_layout(id) {
                        match &field {
                            EditFieldId::NoteTitle => layout
                                .parse_settings::<NoteModel>()
                                .title
                                .unwrap_or_default(),
                            EditFieldId::NoteContent => layout
                                .parse_settings::<NoteModel>()
                                .content
                                .unwrap_or_default(),
                            EditFieldId::ReminderListName => layout
                                .parse_settings::<RemindersListModel>()
                                .list_name
                                .unwrap_or_default(),
                            EditFieldId::ReminderItem(i) => layout
                                .parse_settings::<RemindersListModel>()
                                .reminders
                                .get(*i)
                                .map(|r| r.title.clone())
                                .unwrap_or_default(),
                            EditFieldId::ReminderNew => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    let multiline = matches!(field, EditFieldId::NoteContent);
                    if let Some(win) = st.widget_windows.get_mut(&id) {
                        win.inline_edit = Some(InlineEditState {
                            field,
                            buffer: initial,
                            multiline,
                        });
                        win.invalidate();
                    }
                    drop(st);
                    unsafe {
                        let _ = SetFocus(hwnd);
                    }
                }
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (px, py) = lparam_point(lparam);
            if id != 0
                && let Some(app) = get_global_app()
            {
                let move_action = {
                    let mut st = app.borrow_mut();
                    let lock_size = st.settings.layout.lock_size;
                    let min_dim = st.settings.dimensions.size.max(36);
                    if let Some(win) = st.widget_windows.get_mut(&id) {
                        match win.drag_mode {
                            DragMode::None => {
                                let dx = px as f32 * 96.0 / win.surface.dpi;
                                let dy = py as f32 * 96.0 / win.surface.dpi;
                                let w = win.surface.width as f32 * 96.0 / win.surface.dpi;
                                let h = win.surface.height as f32 * 96.0 / win.surface.dpi;
                                let hr = !lock_size
                                    && dx >= w - RESIZE_GRIP_DIP
                                    && dy >= h - RESIZE_GRIP_DIP;
                                if hr != win.hover_resize {
                                    win.hover_resize = hr;
                                    win.invalidate();
                                }
                                None
                            }
                            DragMode::Move => {
                                let mut pt = POINT::default();
                                unsafe {
                                    let _ = GetCursorPos(&mut pt);
                                }
                                let nx =
                                    win.drag_start_rect.left + (pt.x - win.drag_start_screen.x);
                                let ny = win.drag_start_rect.top + (pt.y - win.drag_start_screen.y);
                                let nw = win.drag_start_rect.right - win.drag_start_rect.left;
                                let nh = win.drag_start_rect.bottom - win.drag_start_rect.top;
                                Some((nx, ny, nw, nh))
                            }
                            DragMode::Resize => {
                                let mut pt = POINT::default();
                                unsafe {
                                    let _ = GetCursorPos(&mut pt);
                                }
                                let min_phys =
                                    ((min_dim as f32) * (win.surface.dpi / 96.0)).round() as i32;
                                let nw = ((win.drag_start_rect.right - win.drag_start_rect.left)
                                    + (pt.x - win.drag_start_screen.x))
                                    .max(min_phys);
                                let nh = ((win.drag_start_rect.bottom - win.drag_start_rect.top)
                                    + (pt.y - win.drag_start_screen.y))
                                    .max(min_phys);
                                Some((win.drag_start_rect.left, win.drag_start_rect.top, nw, nh))
                            }
                        }
                    } else {
                        None
                    }
                };
                if let Some((nx, ny, nw, nh)) = move_action {
                    unsafe {
                        let _ = SetWindowPos(hwnd, HWND_BOTTOM, nx, ny, nw, nh, SWP_NOACTIVATE);
                        // Repaint live during drag so Liquid Glass refracts the wallpaper smoothly!
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if id != 0
                && let Some(app) = get_global_app()
            {
                let finished = {
                    let mut st = app.borrow_mut();
                    st.widget_windows
                        .get_mut(&id)
                        .map(|w| std::mem::replace(&mut w.drag_mode, DragMode::None))
                        .unwrap_or(DragMode::None)
                };
                if finished != DragMode::None {
                    unsafe {
                        let _ = ReleaseCapture();
                    }
                    let mut wrc = RECT::default();
                    let dpi = unsafe {
                        let _ = GetWindowRect(hwnd, &mut wrc);
                        (GetDpiForWindow(hwnd) as f32).max(96.0)
                    };
                    let scale = f64::from(dpi) / 96.0;
                    let mut fx = wrc.left;
                    let mut fy = wrc.top;
                    let mut fw_dip = ((f64::from(wrc.right - wrc.left)) / scale).round() as i32;
                    let mut fh_dip = ((f64::from(wrc.bottom - wrc.top)) / scale).round() as i32;

                    {
                        let mut st = app.borrow_mut();
                        let dims = st.settings.dimensions.clone();
                        if finished == DragMode::Move && st.settings.layout.snap_position {
                            fx = core_rs::port::grid_snap_position(
                                f64::from(fx),
                                i64::from(dims.size),
                                i64::from(dims.margin),
                                scale,
                            )
                            .unwrap_or(i64::from(fx)) as i32;
                            fy = core_rs::port::grid_snap_position(
                                f64::from(fy),
                                i64::from(dims.size),
                                i64::from(dims.margin),
                                scale,
                            )
                            .unwrap_or(i64::from(fy)) as i32;
                        }
                        if finished == DragMode::Resize && st.settings.layout.snap_size {
                            fw_dip = core_rs::port::grid_snap_size(
                                f64::from(fw_dip),
                                i64::from(dims.size),
                                i64::from(dims.margin),
                            )
                            .unwrap_or(i64::from(fw_dip))
                                as i32;
                            fh_dip = core_rs::port::grid_snap_size(
                                f64::from(fh_dip),
                                i64::from(dims.size),
                                i64::from(dims.margin),
                            )
                            .unwrap_or(i64::from(fh_dip))
                                as i32;
                        }
                        if let Some(layout) = st.find_layout_mut(id) {
                            layout.x = fx;
                            layout.y = fy;
                            layout.width = fw_dip;
                            layout.height = fh_dip;
                        }
                        st.save_layouts();
                    }
                    let fw_phys = (f64::from(fw_dip) * scale).round() as i32;
                    let fh_phys = (f64::from(fh_dip) * scale).round() as i32;
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            HWND_BOTTOM,
                            fx,
                            fy,
                            fw_phys,
                            fh_phys,
                            SWP_NOACTIVATE,
                        );
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP | WM_CONTEXTMENU => {
            if id != 0 {
                show_context_menu(hwnd, id);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if id != 0
                && let Some(app) = get_global_app()
            {
                let vk = wparam.0 as u16;
                let ctrl = unsafe { (GetKeyState(i32::from(VK_CONTROL.0)) as u16 & 0x8000) != 0 };
                let is_editing = app
                    .borrow()
                    .widget_windows
                    .get(&id)
                    .and_then(|w| w.inline_edit.as_ref())
                    .map(|e| e.multiline);
                if let Some(multiline) = is_editing {
                    if vk == VK_ESCAPE.0 || vk == VK_TAB.0 || (vk == VK_RETURN.0 && !multiline) {
                        commit_inline_edit(&app, id);
                        return LRESULT(0);
                    }
                    if vk == VK_RETURN.0 && multiline {
                        let mut st = app.borrow_mut();
                        if let Some(win) = st.widget_windows.get_mut(&id) {
                            if let Some(ed) = win.inline_edit.as_mut() {
                                ed.buffer.push('\n');
                            }
                            win.invalidate();
                        }
                        return LRESULT(0);
                    }
                    if vk == VK_BACK.0 {
                        let mut st = app.borrow_mut();
                        if let Some(win) = st.widget_windows.get_mut(&id) {
                            if let Some(ed) = win.inline_edit.as_mut() {
                                ed.buffer.pop();
                            }
                            win.invalidate();
                        }
                        return LRESULT(0);
                    }
                    if ctrl && vk == b'V' as u16 {
                        if let Some(txt) = get_clipboard_text(hwnd) {
                            let mut st = app.borrow_mut();
                            if let Some(win) = st.widget_windows.get_mut(&id) {
                                if let Some(ed) = win.inline_edit.as_mut() {
                                    let cleaned = if ed.multiline {
                                        txt.replace("\r\n", "\n")
                                    } else {
                                        txt.replace(['\r', '\n'], " ")
                                    };
                                    ed.buffer.push_str(&cleaned);
                                }
                                win.invalidate();
                            }
                        }
                        return LRESULT(0);
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CHAR => {
            if id != 0
                && let Some(app) = get_global_app()
                && let Some(ch) = char::from_u32(wparam.0 as u32)
                && !ch.is_control()
            {
                let mut st = app.borrow_mut();
                if let Some(win) = st.widget_windows.get_mut(&id)
                    && let Some(ed) = win.inline_edit.as_mut()
                {
                    ed.buffer.push(ch);
                    win.invalidate();
                    return LRESULT(0);
                }
            }
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            if id != 0
                && let Some(app) = get_global_app()
            {
                commit_inline_edit(&app, id);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
