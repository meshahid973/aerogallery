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
        ]),
    ))
    .expect("failed to build AeroGallery");
}
