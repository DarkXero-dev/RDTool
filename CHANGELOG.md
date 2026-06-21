## [0.1.5] - 2026-06-21

### Features

- Settings save button pinned to bottom of settings panel with separator and decoration
- Global error popup centralizes all errors into one styled modal (warning icon, dismiss button)
- Cross-platform build: arboard now uses wayland-data-control on Linux, default features on Windows/macOS

### Bug Fixes

- Add libxdo-dev to Linux CI deps (arboard/enigo linker failure)
- Remove dead per-page error fields (dl_error, torrent_error, stream_error, webdav_err)

## [0.1.4] - 2026-06-21

### Maintenance

- Remove React/TypeScript/Node.js frontend and legacy Tauri config

## [0.1.3] - 2026-06-21

### Refactoring

- Replace Tauri/WebKit frontend with native egui GUI

### Bug Fixes

- Force XWayland and disable DMABuf renderer to fix WebKit crash on Wayland
- Wayland EGL crash; upgrade git-cliff-action v3 to v4; remove unused signing password env
- Correct process plugin permission name; add WebDAV mount feature

### Features

- Add self-updater, embedded HLS media player, and CI signing
