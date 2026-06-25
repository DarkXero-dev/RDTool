# Maintainer: DarkXero-dev <steve@techxero.com>
pkgname=rdtool
pkgver=0.1.33.pre
pkgrel=1
pkgdesc="Real-Debrid GUI Client"
arch=('x86_64')
url="https://github.com/DarkXero-dev/RDTool"
license=('MIT')
depends=('gtk3' 'openssl' 'xdotool' 'glib2' 'libayatana-appindicator' 'sqlite')
makedepends=('rust' 'cargo' 'wayland')

# 0.1.30.pre -> v0.1.30-pre   |   0.1.30 -> v0.1.30
_tag="v${pkgver/.pre/-pre}"
_srcname="RDTool-${_tag#v}"

source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/${_tag}.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$srcdir/$_srcname/src-tauri"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$srcdir/$_srcname/src-tauri"
    export RUSTUP_TOOLCHAIN=stable
    # XeroLinux makepkg.conf passes -flto in CFLAGS; the cc crate forwards CFLAGS
    # to ring's assembly compiler, producing LTO bitcode that neither lld nor bfd
    # can resolve. Clear CFLAGS/CXXFLAGS to plain -O2 and force gcc linker (bfd).
    export CFLAGS="-O2"
    export CXXFLAGS="-O2"
    export RUSTFLAGS="-C opt-level=2"
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc
    cargo build --release
}

package() {
    cd "$srcdir/$_srcname"

    install -Dm755 src-tauri/target/release/rdtool \
        "$pkgdir/usr/bin/rdtool"

    install -Dm644 src-tauri/icons/icon.png \
        "$pkgdir/usr/share/pixmaps/rdtool.png"

    install -Dm644 src-tauri/icons/128x128.png \
        "$pkgdir/usr/share/icons/hicolor/128x128/apps/rdtool.png"

    install -Dm644 src-tauri/packaging/rdtool.desktop \
        "$pkgdir/usr/share/applications/rdtool.desktop"
}
