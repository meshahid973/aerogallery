use std::mem::size_of;
use std::ptr::null_mut;
use tauri::WebviewWindow;
use windows_sys::Win32::{
    System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    UI::{
        Accessibility::{HIGHCONTRASTW, HCF_HIGHCONTRASTON},
        WindowsAndMessaging::{SystemParametersInfoW, SPI_GETHIGHCONTRAST},
    },
};

fn system_permits_transparency() -> bool {
    let mut contrast: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
    contrast.cbSize = size_of::<HIGHCONTRASTW>() as u32;
    if unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, contrast.cbSize, (&mut contrast as *mut HIGHCONTRASTW).cast(), 0) } == 0
        || contrast.dwFlags & HCF_HIGHCONTRASTON != 0 {
        return false;
    }
    let path: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0".encode_utf16().collect();
    let value: Vec<u16> = "EnableTransparency\0".encode_utf16().collect();
    let mut enabled = 1u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, path.as_ptr(), value.as_ptr(), RRF_RT_REG_DWORD, null_mut(), (&mut enabled as *mut u32).cast(), &mut size)
    };
    // Older Windows installations may not have this setting.
    status != 0 || enabled != 0
}

pub fn apply(window: &WebviewWindow, tint: [u8; 3], opacity: u8, permitted: bool) -> bool {
    if !permitted || opacity >= 100 || !system_permits_transparency() {
        let _ = window_vibrancy::clear_acrylic(window);
        return false;
    }
    let [red, green, blue] = tint;
    if window_vibrancy::apply_acrylic(window, Some((red, green, blue, 48))).is_ok() {
        true
    } else {
        let _ = window_vibrancy::clear_acrylic(window);
        false
    }
}
