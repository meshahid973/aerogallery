# AeroGallery

An offline-first desktop photo gallery with a Frutiger Aero interface, built with Tauri, Rust, React and TypeScript.

Requires Node.js 22.12+, pnpm 10, stable Rust and the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

Use `pnpm build` for frontend checks and `pnpm tauri build -- --locked` for a Windows build.

Browse user-selected folders, filter and sort photos, and resize the thumbnail grid. Rust maintains a persistent SQLite library and reconciles selected folders with debounced native filesystem notifications, targeted directory scans and a periodic fallback. The initial listing is published before metadata enrichment. Two folder workers reconcile directories and two metadata workers probe image headers and EXIF for changed files without decoding pixels, and two bounded thumbnail jobs resize with runtime SIMD acceleration. Thumbnails use a 16 MiB memory cache and a separate SQLite disk cache capped at 512 MiB of encoded payloads or 20,000 entries. The gallery renders viewport rows rather than pages. Supported formats: JPEG, PNG, WebP, GIF, BMP, TIFF, ICO, TGA, QOI and PNM (first frame/page for animated or multipage files). The photo viewer opens with double-click or Enter, supports fit/actual-size zoom, drag panning, temporary rotation, native fullscreen, adjacent-image preloading and a slideshow. Source files are never modified. Full-resolution viewing uses the original dimensions within the existing file/pixel limits; unsupported browser formats are converted to lossless PNG for display (first frame/page).

See [design](DESIGN.md) and [asset notices](ASSETS.md). Native commands are scoped to the main window and the folders chosen through the native picker; no general filesystem, shell or asset-protocol access is exposed.
