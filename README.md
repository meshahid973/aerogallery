# AeroGallery

An offline-first desktop photo gallery with a Frutiger Aero interface, built with Tauri, Rust, React and TypeScript.

## Development

Requires Node.js 22.12+, pnpm 10, stable Rust and the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/), including WebView2 and the MSVC C++ build tools.

```sh
pnpm install
pnpm tauri dev
```

Use `pnpm dev` for the browser-only shell preview, `pnpm build` for frontend checks, and `pnpm tauri build` to build the Windows installer.

## Foundation

The native folder picker and version IPC are connected in code. Folder indexing, thumbnail generation and photo viewing are not implemented yet. The UI deliberately exposes no simulated gallery actions.

Shared colors, geometry, lighting and glass density live in `src/styles/tokens.css`; material treatments live in `src/styles/materials.css`; application layout is separate. New features should reuse these materials and the shared components instead of overriding them globally. See [asset notices](ASSETS.md) for provenance.

The local main window has permission to open the native picker and query the application version only. There is no arbitrary filesystem, shell, network or local asset-protocol API exposed to the frontend.
