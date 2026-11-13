#!/bin/sh
set -eu

project_root="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
build_dir="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mintool-target}"
output_dir="$project_root/dist"
if command -v cargo >/dev/null 2>&1; then
    cargo_bin="$(command -v cargo)"
else
    cargo_bin="$HOME/.cargo/bin/cargo"
fi
for tool in dpkg-deb dpkg-shlibdeps dpkg-architecture desktop-file-validate; do
    command -v "$tool" >/dev/null 2>&1 || { printf '缺少打包工具：%s\n' "$tool" >&2; exit 1; }
done
CARGO_TARGET_DIR="$build_dir" "$cargo_bin" build --release --locked --manifest-path "$project_root/Cargo.toml"
version="$("$build_dir/release/mintool" --version)"
version="${version#Mintool }"
package_version="$version-1"
architecture="$(dpkg-architecture -qDEB_HOST_ARCH)"
mkdir -p "$output_dir"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM
stage="$work_dir/package"
mkdir -p "$stage/DEBIAN" "$stage/usr/bin" "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/256x256/apps" "$stage/usr/share/icons/hicolor/scalable/apps" \
    "$stage/usr/share/doc/mintool" "$work_dir/debian"
install -m 755 "$build_dir/release/mintool" "$stage/usr/bin/mintool"
install -m 644 "$project_root/assets/desktop/mintool.png" "$stage/usr/share/icons/hicolor/256x256/apps/mintool.png"
install -m 644 "$project_root/assets/desktop/mintool.svg" "$stage/usr/share/icons/hicolor/scalable/apps/mintool.svg"
install -m 644 "$project_root/packaging/debian/mintool.desktop" "$stage/usr/share/applications/mintool.desktop"
printf 'X-Mintool-Version=%s\n' "$version" >> "$stage/usr/share/applications/mintool.desktop"
desktop-file-validate "$stage/usr/share/applications/mintool.desktop"
install -m 644 "$project_root/README.md" "$stage/usr/share/doc/mintool/README.md"
install -m 644 "$project_root/assets/LICENSE-LUCIDE" "$stage/usr/share/doc/mintool/LICENSE-LUCIDE"
for dependency in adabraka-gpui xim-ctext zed-xim; do
    mkdir -p "$stage/usr/share/doc/mintool/$dependency"
    for license in "$project_root/vendor/$dependency"/LICENSE*; do
        [ -f "$license" ] || continue
        install -m 644 "$license" "$stage/usr/share/doc/mintool/$dependency/"
    done
done
for script in postinst postrm; do
    install -m 755 "$project_root/packaging/debian/$script" "$stage/DEBIAN/$script"
done
# Let Debian derive versioned shared-library requirements from the actual ELF.
cat > "$work_dir/debian/control" <<CONTROL
Source: mintool
Section: devel
Priority: optional
Maintainer: Mintool Developers <mintool@localhost>

Package: mintool
Architecture: any
Description: Terminal-first project workspace
CONTROL
dependencies="$(cd "$work_dir" && dpkg-shlibdeps -O -e"$stage/usr/bin/mintool")"
dependencies="${dependencies#shlibs:Depends=}"
installed_size="$(du -sk "$stage/usr" | cut -f1)"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: mintool
Version: $package_version
Section: devel
Priority: optional
Architecture: $architecture
Maintainer: Mintool Developers <mintool@localhost>
Installed-Size: $installed_size
Depends: $dependencies, libfontconfig1, libvulkan1, libwayland-client0, libx11-6, git, xdg-utils, libglib2.0-bin, hicolor-icon-theme
Recommends: fonts-noto-cjk, mesa-vulkan-drivers
Description: Terminal-first project workspace
 Native project terminals, split panes, Git changes, a live file explorer,
 file previews, and light/dark themes.
CONTROL
chmod -R go-w "$stage"
package_name="mintool_${package_version}_${architecture}.deb"
dpkg-deb --root-owner-group -Zxz --build "$stage" "$output_dir/$package_name"
(cd "$output_dir" && sha256sum "$package_name" > "$package_name.sha256")
printf '安装包：%s\n' "$output_dir/$package_name"
