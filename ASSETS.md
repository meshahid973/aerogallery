# Asset notices

## Tango Desktop Project

The interface icons are from the Tango Desktop Project's 0.8.90 icon theme, released into the public domain. Rasterized copies are sourced from [nigeltao/tango-icon-library-pngs](https://github.com/nigeltao/tango-icon-library-pngs) ([COPYING](https://github.com/nigeltao/tango-icon-library-pngs/blob/main/COPYING)).

| Local file | Upstream file |
| --- | --- |
| src/assets/icons/picture.png | png-64/mimetypes-image-x-generic.png |
| src/assets/icons/folder.png | png-64/places-folder.png |
| src/assets/icons/open.png | png-64/actions-document-open.png |
| src/assets/icons/up.png | png-64/actions-go-up.png |
| src/assets/icons/back.png | png-64/actions-go-previous.png |
| src/assets/icons/forward.png | png-64/actions-go-next.png |
| src/assets/icons/refresh.png | png-64/actions-view-refresh.png |
| src/assets/icons/remove.png | png-64/actions-list-remove.png |
| src/assets/icons/search.png | png-64/actions-system-search.png |
| src/assets/icons/appearance.png | png-64/categories-preferences-system.png |
| src/assets/icons/play.png | png-64/actions-media-playback-start.png |
| src/assets/icons/pause.png | png-64/actions-media-playback-pause.png |
| src/assets/icons/fullscreen.png | png-64/actions-view-fullscreen.png |
| src/assets/icons/rotate-left.png | png-64/actions-edit-undo.png |
| src/assets/icons/rotate-right.png | png-64/actions-edit-redo.png |
| src-tauri/icons/icon.png | png-64/mimetypes-image-x-generic.png |
| src-tauri/icons/icon.ico | ICO container of png-64/mimetypes-image-x-generic.png |

The native icon is a temporary public-domain picture icon, not an original AeroGallery brand mark.

## Interface references

The CSS is original application code inspired by historical [Windows UX guidance](https://learn.microsoft.com/en-us/windows/win32/uxguide/guidelines), [7.css](https://github.com/khang-nd/7.css), and the [Frutiger Obsidian theme](https://github.com/gelogonza/frutiger-obsidian-theme). No upstream stylesheet, proprietary Windows icons, wallpaper, font files or generated artwork is bundled. The background uses CSS gradients and the UI uses installed system fonts.

The three user-supplied CSS references informed material techniques only. All theme CSS is newly authored; remote images, cursors, fonts and decorative effects in those files are not copied into the app.

## Nature imagery

- `public/appearance/nature.jpg`: **Fall Field**, National Park Service, photographed November 12, 2023 at First State National Historical Park. [Wikimedia Commons source and public-domain notice](https://commons.wikimedia.org/wiki/File:Green_rolling_hills_are_accompanied_by_tall_trees_and_a_blue_sky_in_the_background._(8462d90c-90a0-4138-a3ba-b3aa237e468a).JPG). Resized to 480 × 320 for the offline theme preview.
- `public/appearance/grass.png`: CC0 grass photograph texture, grayscale crop distributed in [scikit-image v0.25.2](https://github.com/scikit-image/scikit-image/blob/v0.25.2/skimage/data/grass.png). [Image-specific CC0 notice](https://scikit-image.org/docs/stable/api/skimage.data.html#skimage.data.grass).

Both are used only in the appearance swatches, not imported into the photo library or painted behind photographs. No runtime network access is needed.
