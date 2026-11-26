#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod disk_cache;
mod imaging;
mod index;
mod library;
mod paths;
mod shell;
#[cfg(windows)]
mod windows_shell;
#[cfg(windows)]
mod native_window;
#[cfg(windows)]
mod taskbar;
#[cfg(windows)]
mod window_placement;

use imaging::PhotoMetadata;
use library::{Library, Listing, Root};
use tauri::{ipc::Response, AppHandle, Emitter, Manager, State};
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
async fn full_image(
    root_id: u32,
    parts: Vec<String>,
    stamp: String,
    state: State<'_, Library>,
) -> Result<Response, String> {
    let library = state.inner().clone();
    let permit = library.view_permit()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        library
            .full_image(root_id, &parts, &stamp)
            .map(Response::new)
    })
    .await
    .map_err(|_| "Cannot open full-resolution picture.".to_string())?
}

#[tauri::command]
async fn viewer_fullscreen(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    #[cfg(windows)] { native_window::fullscreen(&window, enabled) }
    #[cfg(not(windows))] { window.set_fullscreen(enabled).map_err(|_| "Cannot change fullscreen mode.".to_string()) }
}

#[tauri::command]
async fn chrome_regions(window: tauri::WebviewWindow, bounds: Option<[i32; 4]>) -> Result<bool, String> {
    #[cfg(windows)] { native_window::regions(&window, bounds) }
    #[cfg(not(windows))] { let _ = (window, bounds); Ok(false) }
}

#[tauri::command]
async fn native_system_menu(window: tauri::WebviewWindow, cursor: bool) -> Result<(), String> {
    #[cfg(windows)] { native_window::show_menu(&window, cursor) }
    #[cfg(not(windows))] { let _ = (window, cursor); Ok(()) }
}

#[tauri::command]
async fn sync_taskbar(window: tauri::WebviewWindow, active: bool, previous: bool, next: bool, can_play: bool, playing: bool) -> Result<(), String> {
    #[cfg(windows)] { native_window::playback(&window, taskbar::Playback { active, previous, next, can_play, playing }) }
    #[cfg(not(windows))] { let _ = (window, active, previous, next, can_play, playing); Ok(()) }
}

fn main() {
    tauri::Builder::default()
        .manage(shell::ShellInbox::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            shell::enqueue(app, shell::arguments(args.into_iter().map(std::path::PathBuf::from), std::path::Path::new(&cwd)));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .on_webview_event(|webview, event| {
            if webview.label() != "main" { return; }
            if let tauri::WebviewEvent::DragDrop(event) = event {
                match event {
                    tauri::DragDropEvent::Enter { .. } => { let _ = webview.emit("shell-drag", true); }
                    tauri::DragDropEvent::Leave => { let _ = webview.emit("shell-drag", false); }
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        let _ = webview.emit("shell-drag", false);
                        shell::enqueue(webview.app_handle(), paths.clone());
                    }
                    _ => {}
                }
            }
        })
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let library = Library::open(&data).map_err(std::io::Error::other)?;
            app.manage(library);
            #[cfg(windows)] {
                app.manage(native_window::NativeState::default());
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(error) = native_window::install(&window, data.join("window-placement.json")) { eprintln!("Window integration: {error}"); }
                }
            }
            if let Ok(cwd) = std::env::current_dir() {
                shell::enqueue(app.handle(), shell::arguments(std::env::args_os().map(std::path::PathBuf::from), &cwd));
            }
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
            viewer_fullscreen,
            chrome_regions,
            native_system_menu,
            sync_taskbar,
            shell::take_shell_items,
            shell::release_external,
            shell::external_image,
            shell::default_apps,
            shell::set_wallpaper
        ])
        .run(tauri::generate_context!())
        .expect("failed to start AeroGallery");
}
