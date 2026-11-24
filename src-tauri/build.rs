fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "pick_folder",
                "list_roots",
                "read_folder",
                "remove_root",
                "thumbnail",
            ]),
        ),
    )
    .expect("failed to build AeroGallery");
}
