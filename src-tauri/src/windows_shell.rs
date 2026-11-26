use std::{fs, path::Path, os::windows::ffi::OsStrExt};
use windows::{core::{w, PCWSTR}, Win32::{System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, COINIT_APARTMENTTHREADED, CLSCTX_ALL}, UI::{Shell::{DesktopWallpaper, IDesktopWallpaper, DWPOS_CENTER, DWPOS_TILE, DWPOS_STRETCH, DWPOS_FIT, DWPOS_FILL, DWPOS_SPAN, ShellExecuteW}, WindowsAndMessaging::SW_SHOWNORMAL}}};

pub fn default_apps() -> Result<(), String> {
    let result = unsafe { ShellExecuteW(None, w!("open"), w!("ms-settings:defaultapps"), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    if result.0 as isize <= 32 { Err("Cannot open Windows Default Apps.".into()) } else { Ok(()) }
}

pub fn wallpaper(data: &Path, bytes: Vec<u8>, position: &str) -> Result<(), String> {
    let position = match position {
        "center" => DWPOS_CENTER, "tile" => DWPOS_TILE, "stretch" => DWPOS_STRETCH,
        "fit" => DWPOS_FIT, "fill" => DWPOS_FILL, "span" => DWPOS_SPAN,
        _ => return Err("Invalid wallpaper position.".into()),
    };
    let dir = data.join("wallpaper");
    fs::create_dir_all(&dir).map_err(|_| "Cannot create wallpaper storage.")?;
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| "Invalid system time.")?.as_nanos();
    let path = if bytes.starts_with(&[0xff, 0xd8]) {
        let path = dir.join(format!("{time}.jpg"));
        fs::write(&path, bytes).map_err(|_| "Cannot save wallpaper.")?;
        path
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let path = dir.join(format!("{time}.png"));
        fs::write(&path, bytes).map_err(|_| "Cannot save wallpaper.")?;
        path
    } else {
        let path = dir.join(format!("{time}.jpg"));
        image::load_from_memory(&bytes).map_err(|_| "Cannot decode wallpaper.")?.into_rgb8().save(&path).map_err(|_| "Cannot save wallpaper.")?;
        path
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = (|| -> windows::core::Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            struct Apartment;
            impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
            let _apartment = Apartment;
            let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
            desktop.SetWallpaper(PCWSTR::null(), PCWSTR(wide.as_ptr()))?;
            desktop.SetPosition(position)?;
            Ok(())
        }
    })();
    if result.is_err() { return Err("Windows could not set the wallpaper or its position.".into()); }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let old = entry.path();
            if old != path && old.file_stem().and_then(|stem| stem.to_str()).is_some_and(|stem| stem.parse::<u128>().is_ok()) && old.extension().is_some_and(|ext| ext == "jpg" || ext == "png") { let _ = fs::remove_file(old); }
        }
    }
    Ok(())
}
