## [0.1.10] - 2026-06-21

### Bug Fixes

- Load FUSE kernel module in CI before cargo-packager runs linuxdeploy (linuxdeploy is an AppImage, needs FUSE)
- Use APPIMAGE_EXTRACT_AND_RUN=1 for all AppImage operations in CI strip step as FUSE fallback

## [0.1.9] - 2026-06-21

### Features

- Replace placeholder icons with official Real-Debrid logo across all platforms (AppImage, Windows ICO, macOS ICNS, all Windows Store tile sizes)

## [0.1.8] - 2026-06-21

### Bug Fixes

- Strip bundled platform libs (GTK3, wayland-client, X11, GL/EGL, pango, etc.) from AppImage; linuxdeploy bundled Ubuntu 22.04 versions conflicted with newer Wayland compositors (KDE Plasma 6+)
- Remove erroneous WINIT_UNIX_BACKEND=x11 override; app now auto-detects Wayland/X11 correctly via host system libs

## [0.1.7] - 2026-06-21

### Bug Fixes

- Force WINIT_UNIX_BACKEND=x11 at startup to fix AppImage crash on Wayland (glutin EGL rejects Wayland display handle inside AppImage sandbox)

## [0.1.6] - 2026-06-21

### Bug Fixes

- Switch eframe renderer from wgpu to glow (OpenGL) - fixes AppImage crash on systems without Vulkan support
- Add libegl1-mesa-dev and libgl1-mesa-dev to Linux CI deps for OpenGL build

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
