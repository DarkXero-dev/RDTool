## [0.1.22] - 2026-06-21

### Bug Fixes

- Fix crash when enabling system tray on Linux: add gtk::init() in run() and gtk::main_iteration_do() per frame so GTK AppIndicator is properly initialized and pumped

## [0.1.21] - 2026-06-21

### Bug Fixes

- Fix crash on startup: eframe built with default-features=false strips x11 and wayland backends; glutin has no supported display handle and panics. Add x11 and wayland features explicitly.

## [0.1.20] - 2026-06-21

### Bug Fixes

- Add desktop entry to deb, rpm, and Arch packages so app appears in launcher
- Fix all compiler warnings: remove dead code (rd_tab, TorrentTab, field fn, update_schedule), remove unused imports, fix deprecated egui APIs (Panel::top/bottom/left, exact_size, Frame::NONE, SliderClamping, content_rect)

## [0.1.19] - 2026-06-21

### Bug Fixes

- Fix PKGBUILD link failure: rusqlite bundled SQLite conflicts with LLD (Arch default linker); use system SQLite on Linux, keep bundled only for Windows/macOS
- Add libsqlite3-dev to CI Linux deps, sqlite to PKGBUILD depends

## [0.1.18] - 2026-06-21

### Bug Fixes

- Add license = "MIT" to Cargo.toml; cargo-generate-rpm requires it

## [0.1.17] - 2026-06-21

### Bug Fixes

- Fix Linux packaging: cargo-packager 0.11.8 does not support rpm format; switch to cargo-generate-rpm (pure Rust, no rpmbuild needed)

## [0.1.16] - 2026-06-21

### Changes

- Drop AppImage: Linux builds now produce .deb (Debian/Ubuntu) and .rpm (Fedora/RHEL) via cargo-packager
- Add PKGBUILD for Arch Linux (build from source with makepkg)
- Remove fuse/squashfs-tools from CI deps; remove AppImage post-process step entirely

## [0.1.15] - 2026-06-21

### Bug Fixes

- Fix squashfs offset detection: scan forward from ELF end at 4-byte increments instead of checking only 3 alignment candidates; linuxdeploy uses a non-standard alignment

## [0.1.14] - 2026-06-21

### Bug Fixes

- Fix YAML workflow parse error: heredoc content at column 0 exits the run block scalar; replace with printf one-liner to write Python script file

## [0.1.13] - 2026-06-21

### Bug Fixes

- Fix AppImage SEGV: byte-scan for squashfs magic found false positives in ELF data; now use ELF PT_LOAD segment parsing to find exact squashfs offset
- Switch squashfs compression to gzip (matches linuxdeploy default; zstd may not be compiled into the AppImage runtime's squashfuse)
- Add .DirIcon and rdtool.png to AppImage root for icon in file managers
- Rename output to .AppImage (capital I) for correct desktop integration
- Fix artifact glob to match both .AppImage and .appimage

## [0.1.12] - 2026-06-21

### Bug Fixes

- Fix AppImage repack: extract runtime ELF from the AppImage itself instead of downloading external files (GitHub rate-limited both appimagetool and runtime-x86_64 downloads)
- Use find -delete instead of rm glob expansion for stripping bundled libs (more reliable in CI)

## [0.1.11] - 2026-06-21

### Bug Fixes

- Replace AppRun in AppImage to skip LD_LIBRARY_PATH; binary uses host system GTK3/wayland/mesa instead of bundled Ubuntu 22.04 versions that crash on KDE Plasma 6+
- Switch AppImage repack from appimagetool (AppImage, needs FUSE + rate-limited) to squashfs-tools + runtime ELF (plain binary, no FUSE)

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
