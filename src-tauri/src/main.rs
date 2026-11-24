#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod disk_cache;
mod imaging;
mod index;
mod library;
mod paths;

use imaging::PhotoMetadata;
use library::{Library, Listing, Root};
use tauri::{ipc::Response, AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn pick_folder(app: AppHandle, state: State<'_, Library>) -> Result<Option<Root>, String> {
    let library = state.inner().clone();
    let permit = library.picker_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let Some(folder) = app
            .dialog()
            .file()
            .set_title("Select a picture folder")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let path = folder.into_path().map_err(|_| "Choose a local folder.")?;
        library.add(path).map(Some)
    })
    .await
    .map_err(|_| "Cannot open the folder picker.".to_string())?
}

#[tauri::command]
async fn list_roots(state: State<'_, Library>) -> Result<Vec<Root>, String> {
    let library = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.roots())
        .await
        .map_err(|_| "Cannot read the library.".to_string())?
}

#[tauri::command]
async fn read_folder(
    root_id: u32,
    parts: Vec<String>,
    refresh: Option<bool>,
    state: State<'_, Library>,
) -> Result<Listing, String> {
    let library = state.inner().clone();
    let permit = library.listing_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library.list(root_id, &parts, refresh.unwrap_or(false))
    })
    .await
    .map_err(|_| "Cannot read this folder.".to_string())?
}

#[tauri::command]
async fn folder_revision(
    root_id: u32,
    parts: Vec<String>,
    state: State<'_, Library>,
) -> Result<i64, String> {
    let library = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.revision(root_id, &parts))
        .await
        .map_err(|_| "Cannot check folder changes.".to_string())?
}

#[tauri::command]
async fn photo_metadata(
    root_id: u32,
    parts: Vec<String>,
    state: State<'_, Library>,
) -> Result<Option<PhotoMetadata>, String> {
    let library = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.metadata(root_id, &parts))
        .await
        .map_err(|_| "Picture metadata is unavailable.".to_string())?
}

#[tauri::command]
async fn remove_root(root_id: u32, state: State<'_, Library>) -> Result<(), String> {
    let library = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.remove(root_id))
        .await
        .map_err(|_| "Cannot remove the folder.".to_string())?
}

#[tauri::command]
async fn thumbnail(
    root_id: u32,
    parts: Vec<String>,
    state: State<'_, Library>,
) -> Result<Response, String> {
    let library = state.inner().clone();
    let permit = library.decode_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library.thumbnail(root_id, &parts).map(Response::new)
    })
    .await
    .map_err(|_| "Picture unavailable.".to_string())?
}

#[tauri::command]
async fn full_image(root_id: u32, parts: Vec<String>, stamp: String, state: State<'_, Library>) -> Result<Response, String> {
    let library = state.inner().clone();
    let permit = library.view_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library.full_image(root_id, &parts, &stamp).map(Response::new)
    }).await.map_err(|_| "Cannot open full-resolution picture.".to_string())?
}

#[tauri::command]
fn viewer_fullscreen(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    window.set_fullscreen(enabled).map_err(|_| "Cannot change fullscreen mode.".to_string())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let library = Library::open(&data).map_err(std::io::Error::other)?;
            app.manage(library);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_folder,
            list_roots,
            read_folder,
            folder_revision,
            photo_metadata,
            remove_root,
            thumbnail,
            full_image,
            viewer_fullscreen
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AeroGallery");
}
