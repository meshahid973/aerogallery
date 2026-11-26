use std::ptr::null_mut;
use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        UI::{Shell::{ITaskbarList3, TaskbarList, THUMBBUTTON, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_DISABLED, THBF_ENABLED, THBF_HIDDEN}, WindowsAndMessaging::HICON},
    },
};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIcon, DestroyIcon};

pub const PREVIOUS: u32 = 501;
pub const PLAY: u32 = 502;
pub const NEXT: u32 = 503;

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playback {
    pub active: bool,
    pub previous: bool,
    pub next: bool,
    pub can_play: bool,
    pub playing: bool,
}

struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        unsafe { DestroyIcon(self.0.0); }
    }
}

fn icon(bytes: &[u8]) -> Option<Icon> {
    let rgba = image::load_from_memory(bytes).ok()?.resize_exact(16, 16, image::imageops::FilterType::Triangle).into_rgba8();
    let mut colors = [0u8; 16 * 16 * 4];
    let mut mask = [0u8; 16 * 2];
    for y in 0..16 {
        for x in 0..16 {
            let p = rgba.get_pixel(x, 15 - y).0;
            let i = (y as usize * 16 + x as usize) * 4;
            colors[i..i+4].copy_from_slice(&[p[2], p[1], p[0], p[3]]);
            if p[3] == 0 { mask[y as usize * 2 + x as usize / 8] |= 0x80 >> (x % 8); }
        }
    }
    let handle = unsafe { CreateIcon(null_mut(), 16, 16, 1, 32, mask.as_ptr(), colors.as_ptr()) };
    (!handle.is_null()).then_some(Icon(HICON(handle)))
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) { unsafe { CoUninitialize(); } }
}

pub struct Taskbar {
    // Drop COM interfaces and icons before uninitializing their apartment.
    api: ITaskbarList3,
    icons: [Icon; 4],
    added: bool,
    previous: Option<Playback>,
    _apartment: Apartment,
}

impl Taskbar {
    pub fn new() -> Option<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok()?; }
        let apartment = Apartment;
        let api: ITaskbarList3 = unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()? };
        unsafe { api.HrInit().ok()?; }
        let icons = [
            icon(include_bytes!("../../src/assets/icons/back.png"))?,
            icon(include_bytes!("../../src/assets/icons/play.png"))?,
            icon(include_bytes!("../../src/assets/icons/pause.png"))?,
            icon(include_bytes!("../../src/assets/icons/forward.png"))?,
        ];
        Some(Self { api, icons, added: false, previous: None, _apartment: apartment })
    }

    pub fn explorer_restarted(&mut self) {
        self.added = false;
        self.previous = None;
    }

    pub fn update(&mut self, hwnd: HWND, playback: Playback) -> Result<()> {
        if self.added && self.previous == Some(playback) { return Ok(()); }
        let button = |id, icon: &Icon, text: &str, enabled| {
            let mut tip = [0u16; 260];
            for (destination, source) in tip.iter_mut().zip(text.encode_utf16()) { *destination = source; }
            THUMBBUTTON {
                dwMask: THB_FLAGS | THB_ICON | THB_TOOLTIP,
                iId: id,
                iBitmap: 0,
                hIcon: icon.0,
                szTip: tip,
                dwFlags: if !playback.active { THBF_HIDDEN } else if enabled { THBF_ENABLED } else { THBF_DISABLED },
            }
        };
        let buttons = [
            button(PREVIOUS, &self.icons[0], "Previous picture", playback.previous),
            button(PLAY, &self.icons[if playback.playing { 2 } else { 1 }], if playback.playing { "Pause slideshow" } else { "Play slideshow" }, playback.can_play),
            button(NEXT, &self.icons[3], "Next picture", playback.next),
        ];
        unsafe {
            if self.added { self.api.ThumbBarUpdateButtons(hwnd, &buttons)?; }
            else { self.api.ThumbBarAddButtons(hwnd, &buttons)?; self.added = true; }
        }
        self.previous = Some(playback);
        Ok(())
    }
}
