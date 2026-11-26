use crate::{taskbar::{Playback, Taskbar, PREVIOUS, PLAY, NEXT}, window_placement::{self, Placement}};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    sync::{atomic::{AtomicBool, Ordering}, Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows::core::w;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{_TrackMouseEvent, TRACKMOUSEEVENT, TME_LEAVE, TME_NONCLIENT},
        Shell::{DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const PARENT_HOOK: usize = 0x4145524f;
const CHILD_HOOK: usize = 0x41455250;

#[derive(Default)]
pub struct NativeState {
    installed: AtomicBool,
    fullscreen: AtomicBool,
    rect: Mutex<Option<[i32; 4]>>,
    playback: Mutex<Playback>,
}

struct Hook {
    app: AppHandle,
    placement_path: PathBuf,
    placement: RefCell<Option<Placement>>,
    taskbar: RefCell<Option<Taskbar>>,
    hover: Cell<bool>,
    maximized: Cell<bool>,
}

fn message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("AeroGallery.Native.Update").0) })
}

fn menu_message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("AeroGallery.Native.SystemMenu").0) })
}

fn taskbar_created() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated").0) })
}

unsafe fn hook(hwnd: HWND) -> Option<*const Hook> {
    let mut data = 0usize;
    if GetWindowSubclass(hwnd, Some(parent_proc), PARENT_HOOK, &mut data) == 0 || data == 0 { None }
    else { Some(data as *const Hook) }
}

unsafe fn max_hit(hwnd: HWND, data: &Hook, lparam: LPARAM) -> bool {
    let state = data.app.state::<NativeState>();
    if state.fullscreen.load(Ordering::Acquire) { return false; }
    let Some(rect) = state.rect.lock().ok().and_then(|r| *r) else { return false; };
    let mut point = POINT { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 };
    let mut bounds: RECT = std::mem::zeroed();
    if ScreenToClient(hwnd, &mut point) == 0 || GetClientRect(hwnd, &mut bounds) == 0 { return false; }
    rect[0] >= 0 && rect[1] >= 0 && rect[2] <= bounds.right && rect[3] <= bounds.bottom &&
        point.x >= rect[0] && point.x < rect[2] && point.y >= rect[1] && point.y < rect[3]
}

unsafe extern "system" fn child_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, parent: usize) -> LRESULT {
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(child_proc), CHILD_HOOK);
    } else if msg == WM_NCHITTEST {
        let parent = parent as HWND;
        if let Some(data) = hook(parent) {
            if max_hit(parent, &*data, lp) { return HTTRANSPARENT as LRESULT; }
        }
    } else if msg == WM_SYSKEYDOWN && wp == 0x20 && (lp & (1 << 29)) != 0 {
        PostMessageW(parent as HWND, menu_message(), 0, 0);
        return 0;
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn attach_child(hwnd: HWND, parent: LPARAM) -> i32 {
    // Subclassing a different process/thread is unsupported by Windows.
    if GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) == GetCurrentThreadId() {
        let mut data = 0usize;
        if GetWindowSubclass(hwnd, Some(child_proc), CHILD_HOOK, &mut data) == 0 {
            SetWindowSubclass(hwnd, Some(child_proc), CHILD_HOOK, parent as usize);
        }
    }
    1
}

unsafe fn style(hwnd: HWND) {
    let old = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let next = (old | (WS_THICKFRAME | WS_SYSMENU | WS_MAXIMIZEBOX | WS_MINIMIZEBOX) as isize) & !(WS_CAPTION as isize);
    if next != old {
        SetWindowLongPtrW(hwnd, GWL_STYLE, next);
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

unsafe fn update_taskbar(hwnd: HWND, data: &Hook, restart: bool) {
    let state = data.app.state::<NativeState>();
    let Ok(playback) = state.playback.lock().map(|p| *p) else { return; };
    let Ok(mut taskbar) = data.taskbar.try_borrow_mut() else { return; };
    if taskbar.is_none() { *taskbar = Taskbar::new(); }
    if let Some(taskbar) = taskbar.as_mut() {
        if restart { taskbar.explorer_restarted(); }
        let _ = taskbar.update(windows::Win32::Foundation::HWND(hwnd), playback);
    }
}

unsafe fn system_menu(hwnd: HWND, data: &Hook, cursor: bool) {
    if data.app.state::<NativeState>().fullscreen.load(Ordering::Acquire) { return; }
    let menu = GetSystemMenu(hwnd, 0);
    if menu.is_null() { return; }
    let maximized = IsZoomed(hwnd) != 0;
    EnableMenuItem(menu, SC_RESTORE, MF_BYCOMMAND | if maximized { MF_ENABLED } else { MF_GRAYED });
    EnableMenuItem(menu, SC_MAXIMIZE, MF_BYCOMMAND | if maximized { MF_GRAYED } else { MF_ENABLED });
    EnableMenuItem(menu, SC_MOVE, MF_BYCOMMAND | if maximized { MF_GRAYED } else { MF_ENABLED });
    EnableMenuItem(menu, SC_SIZE, MF_BYCOMMAND | if maximized { MF_GRAYED } else { MF_ENABLED });
    let mut point: POINT = std::mem::zeroed();
    if cursor { GetCursorPos(&mut point); }
    else {
        let mut bounds: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut bounds);
        point.x = bounds.left + 8;
        point.y = bounds.top + 32;
    }
    let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, std::ptr::null());
    if command != 0 { PostMessageW(hwnd, WM_SYSCOMMAND, command as WPARAM, 0); }
}

fn hover(data: &Hook, value: bool) {
    if data.hover.replace(value) != value {
        let _ = data.app.emit_to("main", "aero-max-hover", value);
    }
}

unsafe extern "system" fn parent_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, raw: usize) -> LRESULT {
    let data = &*(raw as *const Hook);
    let state = data.app.state::<NativeState>();
    let fullscreen = state.fullscreen.load(Ordering::Acquire);
    if msg == WM_NCDESTROY {
        state.installed.store(false, Ordering::Release);
        RemoveWindowSubclass(hwnd, Some(parent_proc), PARENT_HOOK);
        let result = DefSubclassProc(hwnd, msg, wp, lp);
        drop(Box::from_raw(raw as *mut Hook));
        return result;
    }
    if msg == message() {
        EnumChildWindows(hwnd, Some(attach_child), hwnd as LPARAM);
        update_taskbar(hwnd, data, false);
        return 0;
    }
    if msg == menu_message() { system_menu(hwnd, data, wp != 0); return 0; }
    if msg == taskbar_created() {
        update_taskbar(hwnd, data, true);
    }
    match msg {
        WM_NCHITTEST if max_hit(hwnd, data, lp) => return HTMAXBUTTON as LRESULT,
        WM_NCCALCSIZE if !fullscreen => return 0,
        WM_GETMINMAXINFO if !fullscreen && lp != 0 => {
            let result = DefSubclassProc(hwnd, msg, wp, lp);
            window_placement::constrain_maximized(hwnd, &mut *(lp as *mut MINMAXINFO));
            return result;
        }
        WM_NCMOUSEMOVE => {
            let over = wp == HTMAXBUTTON as usize;
            hover(data, over);
            if over {
                let mut tracking = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE | TME_NONCLIENT, hwndTrack: hwnd, dwHoverTime: 0 };
                _TrackMouseEvent(&mut tracking);
            }
        }
        WM_NCMOUSELEAVE | WM_MOUSEMOVE => hover(data, false),
        WM_NCLBUTTONDOWN if wp == HTMAXBUTTON as usize => return 0,
        WM_NCLBUTTONUP if wp == HTMAXBUTTON as usize => {
            hover(data, false);
            PostMessageW(hwnd, WM_SYSCOMMAND, if IsZoomed(hwnd) != 0 { SC_RESTORE as usize } else { SC_MAXIMIZE as usize }, 0);
            return 0;
        }
        WM_SYSKEYDOWN if wp == 0x20 && (lp & (1 << 29)) != 0 => {
            system_menu(hwnd, data, false);
            return 0;
        }
        WM_COMMAND if (wp >> 16) & 0xffff == 0x1800 => {
            let playback = state.playback.lock().map(|p| *p).unwrap_or_default();
            let action = match (wp & 0xffff) as u32 {
                PREVIOUS if playback.active && playback.previous => Some("previous"),
                PLAY if playback.active && playback.can_play => Some("play"),
                NEXT if playback.active && playback.next => Some("next"),
                _ => None,
            };
            if let Some(action) = action { let _ = data.app.emit_to("main", "aero-taskbar", action); return 0; }
        }
        WM_DPICHANGED => {
            if let Ok(mut rect) = state.rect.lock() { *rect = None; }
        }
        WM_SIZE if !fullscreen => {
            let maximized = IsZoomed(hwnd) != 0;
            if data.maximized.replace(maximized) != maximized {
                if let Ok(mut placement) = data.placement.try_borrow_mut() { window_placement::save(hwnd, &data.placement_path, &mut placement); }
            }
        }
        WM_EXITSIZEMOVE | WM_CLOSE if !fullscreen => {
            if let Ok(mut placement) = data.placement.try_borrow_mut() { window_placement::save(hwnd, &data.placement_path, &mut placement); }
        }
        WM_DISPLAYCHANGE if !fullscreen => {
            let result = DefSubclassProc(hwnd, msg, wp, lp);
            window_placement::recover(hwnd);
            return result;
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

pub fn install(window: &WebviewWindow, path: PathBuf) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|_| "Native window unavailable.")?.0;
    unsafe {
        let placement = window_placement::normal(hwnd);
        let data = Box::new(Hook {
            app: window.app_handle().clone(), placement_path: path, placement: RefCell::new(placement),
            taskbar: RefCell::new(None), hover: Cell::new(false), maximized: Cell::new(IsZoomed(hwnd) != 0),
        });
        let raw = Box::into_raw(data);
        if SetWindowSubclass(hwnd, Some(parent_proc), PARENT_HOOK, raw as usize) == 0 {
            drop(Box::from_raw(raw));
            return Err("Native window hook unavailable.".into());
        }
        window.state::<NativeState>().installed.store(true, Ordering::Release);
        style(hwnd);
        let data = &*raw;
        if let Some(placement) = window_placement::restore(hwnd, &data.placement_path) {
            *data.placement.borrow_mut() = Some(placement);
        }
        EnumChildWindows(hwnd, Some(attach_child), hwnd as LPARAM);
        PostMessageW(hwnd, message(), 0, 0);
    }
    Ok(())
}

pub fn regions(window: &WebviewWindow, bounds: Option<[i32; 4]>) -> Result<bool, String> {
    let state = window.state::<NativeState>();
    if !state.installed.load(Ordering::Acquire) { return Ok(false); }
    let bounds = bounds.filter(|r| r[0] >= 0 && r[1] >= 0 && r[2] > r[0] && r[3] > r[1] && r[2]-r[0] <= 256 && r[3]-r[1] <= 192);
    *state.rect.lock().map_err(|_| "Native window unavailable.")? = bounds;
    let hwnd = window.hwnd().map_err(|_| "Native window unavailable.")?.0;
    unsafe { PostMessageW(hwnd, message(), 0, 0); }
    Ok(true)
}

pub fn show_menu(window: &WebviewWindow, cursor: bool) -> Result<(), String> {
    if !window.state::<NativeState>().installed.load(Ordering::Acquire) { return Err("Window menu unavailable.".into()); }
    let hwnd = window.hwnd().map_err(|_| "Native window unavailable.")?.0;
    unsafe { PostMessageW(hwnd, menu_message(), usize::from(cursor), 0); }
    Ok(())
}

pub fn playback(window: &WebviewWindow, playback: Playback) -> Result<(), String> {
    let state = window.state::<NativeState>();
    if !state.installed.load(Ordering::Acquire) { return Ok(()); }
    let mut old = state.playback.lock().map_err(|_| "Taskbar unavailable.")?;
    if *old == playback { return Ok(()); }
    *old = playback;
    drop(old);
    let hwnd = window.hwnd().map_err(|_| "Native window unavailable.")?.0;
    unsafe { PostMessageW(hwnd, message(), 0, 0); }
    Ok(())
}

pub fn fullscreen(window: &WebviewWindow, enabled: bool) -> Result<(), String> {
    let state = window.state::<NativeState>();
    let prior = state.fullscreen.swap(enabled, Ordering::AcqRel);
    if let Err(_) = window.set_fullscreen(enabled) {
        state.fullscreen.store(prior, Ordering::Release);
        return Err("Cannot change fullscreen mode.".into());
    }
    if !enabled && state.installed.load(Ordering::Acquire) {
        let hwnd = window.hwnd().map_err(|_| "Native window unavailable.")?.0 as usize;
        window.run_on_main_thread(move || unsafe { style(hwnd as HWND); }).map_err(|_| "Cannot restore the caption.".to_string())?;
    }
    Ok(())
}
