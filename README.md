# AeroGallery

An offline-first desktop photo gallery with a Frutiger Aero interface, built with Tauri, Rust, React and TypeScript.

Requires Node.js 22.12+, pnpm 10, stable Rust and the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Use `pnpm build` for frontend checks and `pnpm tauri build -- --locked` for a Windows build.

Browse user-selected folders, filter and sort photos, and resize the thumbnail grid. Rust lists one directory at a time and generates bounded, orientation-corrected thumbnails for JPEG, PNG and WebP. Sources and thumbnail cache are session-only. Persistent indexing, filesystem watching, full-resolution viewing and additional formats remain future phases.

See [design](DESIGN.md) and [asset notices](ASSETS.md). Native commands are scoped to the main window and the folders chosen through the native picker; no general filesystem, shell or asset-protocol access is exposed.
