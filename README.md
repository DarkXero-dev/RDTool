# RDTool

[![Build](https://github.com/DarkXero-dev/RDTool/actions/workflows/build.yml/badge.svg)](https://github.com/DarkXero-dev/RDTool/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/DarkXero-dev/RDTool)](https://github.com/DarkXero-dev/RDTool/releases/latest)

A native desktop GUI client for [Real-Debrid](https://real-debrid.com). Converts restricted hosted links and magnet files into fast, direct downloads. Built with Tauri 2 and Rust - not a web wrapper.

## Features

- **Unrestricted Downloader** - paste links from any supported host, get direct download URLs instantly
- **Torrent Support** - add magnet links or `.torrent` files, monitor progress, grab generated links
- **Streaming** - generate HLS, DASH, MP4, and WebM transcode URLs for video content
- **Built-in Download Manager** - multi-threaded chunked HTTP downloads with a persistent queue
- **Scheduler** - set start times per download, configure quiet hours globally
- **Export Links** - save generated URLs to `.txt` for use in external download managers
- **Secure Token Storage** - API token encrypted with AES-256-GCM using a machine-derived key; never stored in plain text

## Download

Grab the latest release for your platform from the [Releases](https://github.com/DarkXero-dev/RDTool/releases/latest) page.

| Platform | File |
|----------|------|
| Linux (Arch, Ubuntu, Fedora, ...) | `.AppImage` |
| Windows | `.exe` (NSIS installer) |
| macOS | `.dmg` |

### Linux (AppImage)

```bash
chmod +x RDTool_*.AppImage
./RDTool_*.AppImage
```

### Arch Linux

Run the `.AppImage` directly. No AUR package needed.

## Getting Your API Token

1. Log in to [real-debrid.com](https://real-debrid.com)
2. Go to [real-debrid.com/apitoken](https://real-debrid.com/apitoken)
3. Copy your token
4. Paste it into RDTool on first launch

## Build from Source

**Prerequisites:**

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18+
- Linux only: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

```bash
git clone https://github.com/DarkXero-dev/RDTool.git
cd RDTool
npm install
npm run tauri dev        # development mode
npm run tauri build      # production build
```

Build output is in `src-tauri/target/release/bundle/`.

## Architecture

RDTool uses a strict security boundary: the React frontend never touches the Real-Debrid API or your token directly. All HTTP requests go through Rust via Tauri's `invoke()` system. The token is decrypted in memory only when a request is made, then discarded.

```
React (UI only)  -->  invoke()  -->  Rust core  -->  api.real-debrid.com
```

## Contributing

Pull requests are welcome. Please use [Conventional Commits](https://www.conventionalcommits.org) so the changelog generates correctly.

```
feat: add something new
fix: correct a bug
perf: make something faster
docs: update documentation
chore: maintenance tasks
```

## License

[MIT](LICENSE) - Copyright (c) 2026 DarkXero-dev
