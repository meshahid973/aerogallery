use std::{
    fs::{self, Metadata},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

pub fn is_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub fn valid_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    !name.is_empty()
        && !name.contains(['/', '\\', ':', '\0'])
        && matches!(components.next(), Some(Component::Normal(part)) if part == name)
        && components.next().is_none()
}

pub fn allowed_photo(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| {
        ["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "ico", "tga", "qoi", "pnm", "ppm", "pgm", "pbm"]
            .iter().any(|format| extension.eq_ignore_ascii_case(format))
    })
}

pub fn relative(parts: &[String]) -> Result<String, String> {
    if parts.len() > 128 || parts.iter().any(|part| !valid_component(part)) {
        return Err("Invalid folder or picture.".into());
    }
    Ok(parts.join("/"))
}

pub fn resolve(root: &Path, parts: &[String]) -> Result<PathBuf, String> {
    relative(parts)?;
    let unavailable = || "The selected item is no longer available.".to_string();
    let metadata = fs::symlink_metadata(root).map_err(|_| unavailable())?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(unavailable());
    }
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
        if is_link(&fs::symlink_metadata(&path).map_err(|_| unavailable())?) {
            return Err("Linked folders and pictures are not supported.".into());
        }
    }
    let canonical = fs::canonicalize(&path).map_err(|_| unavailable())?;
    if !canonical.starts_with(root) {
        return Err("The selected item is outside the library.".into());
    }
    Ok(canonical)
}

pub fn fingerprint(metadata: &Metadata) -> (String, u64) {
    let modified = metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    let created = metadata.created().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    let stamp = format!("{}:{}:{}", metadata.len(), modified.map_or(0, |time| time.as_nanos()), created.map_or(0, |time| time.as_nanos()));
    (stamp, modified.map_or(0, |time| time.as_secs()))
}

#[cfg(windows)]
pub fn matches_open_path(file:&fs::File,expected:&Path)->bool {
    use std::os::windows::{ffi::OsStringExt,io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;
    let handle=file.as_raw_handle();
    let size=unsafe{GetFinalPathNameByHandleW(handle,std::ptr::null_mut(),0,0)};
    if size==0||size>32_768 { return false; }
    let mut buffer=vec![0u16;size as usize];
    let len=unsafe{GetFinalPathNameByHandleW(handle,buffer.as_mut_ptr(),size,0)};
    if len==0||len>=size { return false; }
    let actual=PathBuf::from(std::ffi::OsString::from_wide(&buffer[..len as usize]));
    actual==expected
}

#[cfg(not(windows))]
pub fn matches_open_path(file:&fs::File,expected:&Path)->bool {
    let Ok(actual)=fs::canonicalize(expected) else { return false; };
    if actual!=expected { return false; }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (Ok(open),Ok(current))=(file.metadata(),fs::symlink_metadata(expected)) else { return false; };
        !is_link(&current)&&open.dev()==current.dev()&&open.ino()==current.ino()
    }
    #[cfg(not(unix))]
    { let _=file; true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_escape_components() {
        for name in ["", ".", "..", "../outside", "a/b", "a\\b", "C:drive", "photo.jpg:stream", "\0"] {
            assert!(!valid_component(name), "{name:?}");
        }
        assert!(valid_component("Summer coast.jpg"));
        assert!(valid_component("写真"));
    }
}
