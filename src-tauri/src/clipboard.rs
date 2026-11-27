use std::{ptr, thread, time::Duration};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HGLOBAL, HWND},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
    },
};

const CF_DIBV5: u32 = 17;
const HEADER_SIZE: usize = 124;

fn header(width: u32, height: u32, bytes: u32) -> [u8; HEADER_SIZE] {
    let mut result = [0; HEADER_SIZE];
    let mut field = |offset: usize, value: u32| {
        result[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    field(0, HEADER_SIZE as u32);
    field(4, width);
    field(8, (-(height as i32)) as u32);
    field(12, 1 | (32 << 16));
    field(16, 3); // BI_BITFIELDS
    field(20, bytes);
    field(40, 0x00ff0000);
    field(44, 0x0000ff00);
    field(48, 0x000000ff);
    field(52, 0xff000000);
    field(56, 0x73524742); // LCS_sRGB
    field(108, 4); // LCS_GM_IMAGES
    result
}

struct Memory(HGLOBAL);
impl Drop for Memory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}

struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

pub fn copy(hwnd: HWND, rgba: image::RgbaImage) -> Result<(), String> {
    let (width, height) = rgba.dimensions();
    let mut pixels = rgba.into_raw();
    let bytes = u32::try_from(pixels.len()).map_err(|_| "Picture too large.")?;
    let size = HEADER_SIZE + pixels.len();
    let mut memory = Memory(unsafe { GlobalAlloc(GMEM_MOVEABLE, size) });
    if memory.0.is_null() {
        return Err("Not enough memory to copy picture.".into());
    }
    let data = unsafe { GlobalLock(memory.0) }.cast::<u8>();
    if data.is_null() {
        return Err("Clipboard memory unavailable.".into());
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    unsafe {
        ptr::copy_nonoverlapping(header(width, height, bytes).as_ptr(), data, HEADER_SIZE);
        ptr::copy_nonoverlapping(pixels.as_ptr(), data.add(HEADER_SIZE), pixels.len());
        GlobalUnlock(memory.0);
    }
    drop(pixels);

    // Another app may briefly own the clipboard. Retry off the UI thread.
    let mut opened = false;
    for _ in 0..5 {
        if unsafe { OpenClipboard(hwnd) } != 0 {
            opened = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    if !opened {
        return Err("Clipboard is busy. Try again.".into());
    }
    let _clipboard = Clipboard;
    if unsafe { EmptyClipboard() } == 0
        || unsafe { SetClipboardData(CF_DIBV5, memory.0) }.is_null()
    {
        return Err("Cannot copy picture to the clipboard.".into());
    }
    // Windows now owns this allocation.
    memory.0 = ptr::null_mut();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_header_preserves_dimensions_and_alpha() {
        let value = header(2, 3, 24);
        let field =
            |offset: usize| u32::from_le_bytes(value[offset..offset + 4].try_into().unwrap());
        assert_eq!(field(0), 124);
        assert_eq!(field(4), 2);
        assert_eq!(field(8) as i32, -3);
        assert_eq!(field(12), 1 | (32 << 16));
        assert_eq!(field(20), 24);
        assert_eq!(field(52), 0xff000000);
    }
}
