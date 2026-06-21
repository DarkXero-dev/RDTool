# Maintainer: DarkXero-dev <steve@techxero.com>
pkgname=rdtool
pkgver=0.1.19
pkgrel=1
pkgdesc="Real-Debrid GUI Client"
arch=('x86_64')
url="https://github.com/DarkXero-dev/RDTool"
license=('MIT')
depends=('gtk3' 'openssl' 'libxdo' 'glib2' 'libayatana-appindicator' 'sqlite')
makedepends=('rust' 'cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$srcdir/RDTool-$pkgver/src-tauri"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$srcdir/RDTool-$pkgver/src-tauri"
    export RUSTUP_TOOLCHAIN=stable
    cargo build --release
}

package() {
    cd "$srcdir/RDTool-$pkgver"

    install -Dm755 src-tauri/target/release/rdtool \
        "$pkgdir/usr/bin/rdtool"

    install -Dm644 src-tauri/icons/icon.png \
        "$pkgdir/usr/share/pixmaps/rdtool.png"

    install -Dm644 src-tauri/icons/128x128.png \
        "$pkgdir/usr/share/icons/hicolor/128x128/apps/rdtool.png"

    install -Dm644 /dev/stdin \
        "$pkgdir/usr/share/applications/rdtool.desktop" << 'EOF'
[Desktop Entry]
Name=RDTool
Comment=Real-Debrid GUI Client
Exec=rdtool
Icon=rdtool
Terminal=false
Type=Application
Categories=Network;FileTransfer;
Keywords=debrid;download;torrent;
EOF
}
