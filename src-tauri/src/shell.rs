use crate::{
    imaging,
    index::{Photo, Root},
    library::Library,
    paths::{self, fingerprint, is_link},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{ipc::Response, AppHandle, Emitter, Manager, State};

const MAX_SELECTION: usize = 4096;

#[derive(Default)]
pub struct ShellInbox {
    inner: Mutex<Inbox>,
    processing: Mutex<()>,
}

#[derive(Default)]
struct Inbox {
    pending: Vec<PathBuf>,
    overflow: bool,
    sequence: u64,
    files: HashMap<String, PathBuf>,
}

#[derive(Serialize)]
pub struct Selection {
    session: String,
    roots: Vec<Root>,
    photos: Vec<Photo>,
    rejected: usize,
}

pub fn enqueue(app: &AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let inbox = app.state::<ShellInbox>();
    if let Ok(mut state) = inbox.inner.lock() {
        for path in paths {
            if state.pending.len() >= MAX_SELECTION {
                state.overflow = true;
                break;
            }
            state.pending.push(path);
        }
    }
    let _ = app.emit_to("main", "shell-selection", ());
}

pub fn arguments(args: impl IntoIterator<Item = PathBuf>, cwd: &Path) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        })
        .collect()
}

fn selected_file(library: &Library, path: &Path) -> Result<(PathBuf, fs::Metadata), String> {
    let original = fs::symlink_metadata(path).map_err(|_| "Picture unavailable.")?;
    if is_link(&original) || !original.is_file() || !paths::allowed_photo(path) {
        return Err("Unsupported or unavailable picture.".into());
    }
    let path = library.external_path(path)?;
    let meta = fs::symlink_metadata(&path).map_err(|_| "Picture unavailable.")?;
    if is_link(&meta) || !meta.is_file() || meta.len() > imaging::MAX_FILE_BYTES {
        return Err("Picture unavailable or too large.".into());
    }
    imaging::open_checked(&path)?;
    Ok((path, meta))
}

#[tauri::command]
pub async fn take_shell_items(
    app: AppHandle,
    library: State<'_, Library>,
) -> Result<Option<Selection>, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let inbox = app.state::<ShellInbox>();
        let _gate = inbox
            .processing
            .lock()
            .map_err(|_| "Selection unavailable.")?;
        let (pending, overflow) = {
            let mut state = inbox.inner.lock().map_err(|_| "Selection unavailable.")?;
            (
                std::mem::take(&mut state.pending),
                std::mem::take(&mut state.overflow),
            )
        };
        if pending.is_empty() && !overflow {
            return Ok(None);
        }
        let mut roots = Vec::new();
        let mut photos = Vec::new();
        let mut files = HashMap::new();
        let mut seen = HashSet::new();
        let mut rejected = usize::from(overflow);
        for path in pending {
            let Ok(meta) = fs::symlink_metadata(&path) else {
                rejected += 1;
                continue;
            };
            if is_link(&meta) {
                rejected += 1;
                continue;
            }
            if meta.is_dir() {
                match library.add(path) {
                    Ok(root) => {
                        if !roots.iter().any(|item: &Root| item.id == root.id) {
                            roots.push(root);
                        }
                    }
                    Err(_) => rejected += 1,
                }
                continue;
            }
            match selected_file(&library, &path) {
                Ok((path, meta)) => {
                    if !seen.insert(path.clone()) {
                        continue;
                    }
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        rejected += 1;
                        continue;
                    };
                    let mut label = name.to_owned();
                    let mut suffix = 2;
                    while files.contains_key(&label) {
                        label = format!("{name} ({suffix})");
                        suffix += 1;
                    }
                    let (stamp, modified) = fingerprint(&meta);
                    photos.push(Photo {
                        name: label.clone(),
                        bytes: meta.len(),
                        modified,
                        stamp,
                        width: None,
                        height: None,
                    });
                    files.insert(label, path);
                }
                Err(_) => rejected += 1,
            }
        }
        let mut state = inbox.inner.lock().map_err(|_| "Selection unavailable.")?;
        let session = if files.is_empty() {
            String::new()
        } else {
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or("Selection limit reached.")?;
            state.files = files;
            state.sequence.to_string()
        };
        Ok(Some(Selection {
            session,
            roots,
            photos,
            rejected,
        }))
    })
    .await
    .map_err(|_| "Cannot open selected items.".to_string())?
}

fn selected_path(inbox: &ShellInbox, session: &str, name: &str) -> Result<PathBuf, String> {
    let state = inbox.inner.lock().map_err(|_| "Selection unavailable.")?;
    if state.sequence.to_string() != session {
        return Err("Selection has expired.".into());
    }
    state
        .files
        .get(name)
        .cloned()
        .ok_or_else(|| "Picture was not selected.".into())
}

fn external_bytes(
    inbox: &ShellInbox,
    session: &str,
    name: &str,
    stamp: &str,
) -> Result<Vec<u8>, String> {
    let path = selected_path(inbox, session, name)?;
    let current = || {
        fs::symlink_metadata(&path)
            .ok()
            .filter(|meta| !is_link(meta) && meta.is_file())
            .map(|meta| fingerprint(&meta).0)
    };
    if current().as_deref() != Some(stamp) {
        return Err("Picture changed. Open it again.".into());
    }
    let bytes = imaging::full_image(&path)?;
    if selected_path(inbox, session, name)? != path || current().as_deref() != Some(stamp) {
        return Err("Picture changed while opening.".into());
    }
    Ok(bytes)
}

#[tauri::command]
pub fn release_external(session: String, inbox: State<'_, ShellInbox>) {
    if let Ok(mut state) = inbox.inner.lock() {
        if state.sequence.to_string() == session {
            state.files.clear();
        }
    }
}

#[tauri::command]
pub async fn external_image(
    app: AppHandle,
    session: String,
    name: String,
    stamp: String,
    library: State<'_, Library>,
) -> Result<Response, String> {
    let permit = library.view_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        external_bytes(&app.state::<ShellInbox>(), &session, &name, &stamp).map(Response::new)
    })
    .await
    .map_err(|_| "Cannot open selected picture.".to_string())?
}

#[tauri::command]
pub fn default_apps() -> Result<(), String> {
    #[cfg(windows)]
    {
        crate::windows_shell::default_apps()
    }
    #[cfg(not(windows))]
    {
        Err("Windows only.".into())
    }
}

#[tauri::command]
pub async fn set_wallpaper(
    app: AppHandle,
    root_id: u32,
    parts: Vec<String>,
    stamp: String,
    position: String,
    library: State<'_, Library>,
) -> Result<(), String> {
    let library = library.inner().clone();
    let permit = library.view_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let bytes = if root_id == 0 {
            if parts.len() != 2 {
                return Err("Invalid selection.".into());
            }
            external_bytes(&app.state::<ShellInbox>(), &parts[0], &parts[1], &stamp)?
        } else {
            library.full_image(root_id, &parts, &stamp)?
        };
        #[cfg(windows)]
        {
            crate::windows_shell::wallpaper(
                &app.path()
                    .app_data_dir()
                    .map_err(|_| "Storage unavailable.")?,
                bytes,
                &position,
            )
        }
        #[cfg(not(windows))]
        {
            let _ = (bytes, position);
            Err("Windows only.".into())
        }
    })
    .await
    .map_err(|_| "Cannot set wallpaper.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_preserve_spaces_and_unicode() {
        let cwd = PathBuf::from("pictures");
        let args = arguments(
            [
                PathBuf::from("aerogallery"),
                PathBuf::from("写真 1.png"),
                PathBuf::from("two.jpg"),
            ],
            &cwd,
        );
        assert_eq!(args, vec![cwd.join("写真 1.png"), cwd.join("two.jpg")]);
    }
    #[test]
    fn external_selection_does_not_grant_arbitrary_paths() {
        let inbox = ShellInbox::default();
        assert!(selected_path(&inbox, "0", "anything.jpg").is_err());
        assert!(selected_path(&inbox, "1", "anything.jpg").is_err());
    }
}
