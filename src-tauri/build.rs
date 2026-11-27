fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "pick_folder",
            "list_roots",
            "read_folder",
            "folder_revision",
            "photo_metadata",
            "remove_root",
            "thumbnail",
            "full_image",
            "viewer_fullscreen",
            "chrome_regions",
            "native_system_menu",
            "sync_taskbar",
            "apply_material",
            "open_release",
            "take_shell_items",
            "release_external",
            "external_image",
            "default_apps",
            "copy_image",
            "set_wallpaper",
        ]),
    ))
    .expect("failed to build AeroGallery");
}
