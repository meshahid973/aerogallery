#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod library;

use library::{Library, Listing, Root};
use tauri::{ipc::Response, AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
async fn pick_folder(app: AppHandle, state: State<'_, Library>) -> Result<Option<Root>, String> {
    let library = state.inner().clone();
    let permit = library.picker_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let Some(folder) = app.dialog().file().set_title("Select a picture folder").blocking_pick_folder() else {
            return Ok(None);
        };
        let path = folder.into_path().map_err(|_| "Choose a local folder.")?;
        library.add(path).map(Some)
    })
    .await
    .map_err(|_| "Cannot open the folder picker.".to_string())?
}

#[tauri::command]
fn list_roots(state: State<'_, Library>) -> Result<Vec<Root>, String> {
    state.roots()
}

#[tauri::command]
async fn read_folder(root_id: u32, parts: Vec<String>, state: State<'_, Library>) -> Result<Listing, String> {
    let library = state.inner().clone();
    let permit = library.listing_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library.list(root_id, &parts)
    })
    .await
    .map_err(|_| "Cannot read this folder.".to_string())?
}

#[tauri::command]
fn remove_root(root_id: u32, state: State<'_, Library>) -> Result<(), String> {
    state.remove(root_id)
}

#[tauri::command]
async fn thumbnail(root_id: u32, parts: Vec<String>, state: State<'_, Library>) -> Result<Response, String> {
    let library = state.inner().clone();
    let permit = library.decode_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library.thumbnail(root_id, &parts).map(Response::new)
    })
    .await
    .map_err(|_| "Picture unavailable.".to_string())?
}

fn main() {
    tauri::Builder::default()
        .manage(Library::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![pick_folder, list_roots, read_folder, remove_root, thumbnail])
        .run(tauri::generate_context!())
        .expect("failed to start AeroGallery");
}
