use serde::{Deserialize, Serialize};
use std::{fs, path::Path, mem::size_of};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsZoomed, SetWindowPos, ShowWindow, SW_MAXIMIZE, SWP_NOACTIVATE, SWP_NOZORDER},
};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Placement {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    maximized: bool,
}

fn rect(p: Placement) -> RECT {
    RECT { left: p.x, top: p.y, right: p.x.saturating_add(p.width), bottom: p.y.saturating_add(p.height) }
}

fn work_area(rect: &RECT) -> Option<RECT> {
    unsafe {
        let monitor = MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        (GetMonitorInfoW(monitor, &mut info) != 0).then_some(info.rcWork)
    }
}

pub fn normal(hwnd: HWND) -> Option<Placement> {
    unsafe {
        if IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 { return None; }
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 { return None; }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        (width > 0 && height > 0).then_some(Placement { x: rect.left, y: rect.top, width, height, maximized: false })
    }
}

fn fit(mut p: Placement, work: RECT) -> Placement {
    let width = (work.right - work.left).max(1);
    let height = (work.bottom - work.top).max(1);
    p.width = p.width.clamp(1, width);
    p.height = p.height.clamp(1, height);
    let intersects = p.x.saturating_add(p.width) > work.left && p.x < work.right && p.y.saturating_add(p.height) > work.top && p.y < work.bottom;
    if !intersects {
        p.x = work.left + (width - p.width) / 2;
        p.y = work.top + (height - p.height) / 2;
    } else {
        p.x = p.x.clamp(work.left, work.right - p.width);
        p.y = p.y.clamp(work.top, work.bottom - p.height);
    }
    p
}

pub fn restore(hwnd: HWND, path: &Path) -> Option<Placement> {
    let data = fs::read(path).ok().filter(|bytes| bytes.len() <= 4096)?;
    let p: Placement = serde_json::from_slice(&data).ok()?;
    if p.width <= 0 || p.height <= 0 { return None; }
    let p = fit(p, work_area(&rect(p))?);
    unsafe {
        SetWindowPos(hwnd, std::ptr::null_mut(), p.x, p.y, p.width, p.height, SWP_NOACTIVATE | SWP_NOZORDER);
        if p.maximized { ShowWindow(hwnd, SW_MAXIMIZE); }
    }
    Some(p)
}

pub fn save(hwnd: HWND, path: &Path, last: &mut Option<Placement>) {
    unsafe {
        if IsIconic(hwnd) != 0 { return; }
        if let Some(current) = normal(hwnd) { *last = Some(current); }
        let Some(mut placement) = *last else { return; };
        placement.maximized = IsZoomed(hwnd) != 0;
        *last = Some(placement);
        if let Ok(data) = serde_json::to_vec(&placement) {
            let temporary = path.with_extension("tmp");
            if fs::write(&temporary, data).is_ok() { let _ = fs::rename(temporary, path); }
        }
    }
}

pub fn recover(hwnd: HWND) {
    let Some(p) = normal(hwnd) else { return; };
    let Some(work) = work_area(&rect(p)) else { return; };
    let next = fit(p, work);
    if next.x != p.x || next.y != p.y || next.width != p.width || next.height != p.height {
        unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), next.x, next.y, next.width, next.height, SWP_NOACTIVATE | SWP_NOZORDER); }
    }
}

pub unsafe fn constrain_maximized(hwnd: HWND, info: &mut windows_sys::Win32::UI::WindowsAndMessaging::MINMAXINFO) {
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut bounds: MONITORINFO = std::mem::zeroed();
    bounds.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut bounds) != 0 {
        info.ptMaxPosition.x = bounds.rcWork.left - bounds.rcMonitor.left;
        info.ptMaxPosition.y = bounds.rcWork.top - bounds.rcMonitor.top;
        info.ptMaxSize.x = bounds.rcWork.right - bounds.rcWork.left;
        info.ptMaxSize.y = bounds.rcWork.bottom - bounds.rcWork.top;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_monitor_recovers_to_work_area() {
        let p = Placement { x: -4000, y: 300, width: 1100, height: 720, maximized: true };
        let fitted = fit(p, RECT { left: 0, top: 0, right: 1920, bottom: 1040 });
        assert_eq!((fitted.x, fitted.y, fitted.width, fitted.height), (410, 160, 1100, 720));
        assert!(fitted.maximized);
    }
}
