# Maintainer: limbsjones <limbsjones@users.noreply.github.com>
# Contributor: limbsjones

pkgname=obsidian-launcher
pkgver=1.0.0
pkgrel=1
pkgdesc="Keyboard-driven search launcher for Obsidian vaults"
arch=('x86_64')
url="https://github.com/limbsjones/obsidian-launcher"
license=('MIT')
depends=(
    'glibc'
    'gcc-libs'
    'libxkbcommon'
    'wayland'
    'xdg-utils'
    'xdotool'
    'wmctrl'
)
makedepends=('cargo' 'git')
optdepends=(
    'obsidian: the note-taking app (optional, opens notes in Obsidian)'
)
source=("$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$srcdir/$pkgname-$pkgver"
}

build() {
    cd "$srcdir/$pkgname-$pkgver"
    export CARGO_TARGET_DIR="$srcdir/target"
    cargo build --release --frozen
}

check() {
    cd "$srcdir/$pkgname-$pkgver"
    export CARGO_TARGET_DIR="$srcdir/target"
    cargo test --frozen
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    export CARGO_TARGET_DIR="$srcdir/target"

    # Binaries
    install -Dm755 "$CARGO_TARGET_DIR/release/obsidian-launcher" \
        "$pkgdir/usr/bin/obsidian-launcher"
    install -Dm755 "$CARGO_TARGET_DIR/release/obsidian-hotkey-daemon" \
        "$pkgdir/usr/bin/obsidian-hotkey-daemon"

    # Desktop entry
    install -Dm644 obsidian-launcher.desktop \
        "$pkgdir/usr/share/applications/obsidian-launcher.desktop"

    # Icon (converted from Obsidian SVG)
    install -Dm644 "$srcdir/obsidian-launcher.png" \
        "$pkgdir/usr/share/icons/hicolor/256x256/apps/obsidian-launcher.png" 2>/dev/null || true

    # Systemd user service
    install -Dm644 "$srcdir/obsidian-hotkey-daemon.service" \
        "$pkgdir/usr/lib/systemd/user/obsidian-hotkey-daemon.service"

    # Manpage (placeholder)
    # install -Dm644 "$srcdir/doc/obsidian-launcher.1" \
    #     "$pkgdir/usr/share/man/man1/obsidian-launcher.1"
}
