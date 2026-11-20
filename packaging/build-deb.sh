#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

arch=$(dpkg --print-architecture)
case "$arch" in
    amd64) target=x86_64-unknown-linux-musl ;;
    arm64) target=aarch64-unknown-linux-musl ;;
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac
rustup target add "$target"
cargo build --locked --release --target "$target"
version=$(cargo pkgid | cut -d '#' -f 2 | sed 's/.*@//')

stage=$(mktemp -d)
chmod 755 "$stage"
trap 'rm -rf "$stage"' EXIT
install -Dm755 "target/$target/release/wireceptor" "$stage/usr/bin/wireceptor"
install -Dm755 packaging/wg-quick-control "$stage/usr/libexec/wireceptor/wg-quick-control"
install -Dm644 packaging/io.github.hrnxm.wireceptor.policy "$stage/usr/share/polkit-1/actions/io.github.hrnxm.wireceptor.policy"
install -Dm644 packaging/50-wireceptor.rules "$stage/usr/share/polkit-1/rules.d/50-wireceptor.rules"
# Ubuntu 22.04 uses the older polkit Local Authority.
install -Dm644 packaging/wireceptor.pkla "$stage/var/lib/polkit-1/localauthority/10-vendor.d/wireceptor.pkla"
for dir in usr/share/applications etc/xdg/autostart; do
    install -Dm644 packaging/wireceptor.desktop "$stage/$dir/wireceptor.desktop"
done
icons="$stage/usr/share/icons/hicolor"
install -Dm644 assets/logo.png "$icons/512x512/apps/wireceptor.png"
for state in on off loading; do
    install -Dm644 "assets/$state.png" "$icons/128x128/status/wireceptor-$state.png"
done

mkdir -p "$stage/DEBIAN" dist
cat > "$stage/DEBIAN/control" <<CONTROL
Package: wireceptor
Version: $version
Architecture: $arch
Maintainer: Harun Muderizovic <hrnxm@users.noreply.github.com>
Section: net
Priority: optional
Depends: dbus, network-manager | wireguard-tools, systemd, pkexec | policykit-1
Description: Desktop panel indicator for WireGuard connections
 Monitor and toggle NetworkManager profiles and systemd wg-quick services.
CONTROL
echo /etc/xdg/autostart/wireceptor.desktop > "$stage/DEBIAN/conffiles"
dpkg-deb --root-owner-group -Zxz --build "$stage" "dist/wireceptor_${version}_${arch}.deb"
