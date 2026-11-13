#!/bin/sh
set -eu

project_root="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
build_dir="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mintool-target}"
install_bin="$HOME/.local/bin"
install_data="${XDG_DATA_HOME:-$HOME/.local/share}"

if command -v cargo >/dev/null 2>&1; then
    cargo_bin="$(command -v cargo)"
else
    cargo_bin="$HOME/.cargo/bin/cargo"
fi
CARGO_TARGET_DIR="$build_dir" "$cargo_bin" build --release --locked --manifest-path "$project_root/Cargo.toml"

# Replace atomically so an already running instance can keep using its binary.
mkdir -p "$install_bin" "$install_data/applications" "$install_data/icons/hicolor/scalable/apps"
temporary_binary="$(mktemp "$install_bin/.mintool.XXXXXX")"
trap 'rm -f "$temporary_binary"' EXIT HUP INT TERM
install -m 755 "$build_dir/release/mintool" "$temporary_binary"
version="$("$temporary_binary" --version)"
version="${version#Mintool }"
mv -f "$temporary_binary" "$install_bin/mintool"
install -m 644 "$project_root/assets/desktop/mintool.svg" "$install_data/icons/hicolor/scalable/apps/mintool.svg"

# Escape the executable path according to the desktop-entry Exec quoting rules.
escaped_binary="$(printf '%s' "$install_bin/mintool" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')"
cat > "$install_data/applications/mintool.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Mintool
Comment=Terminal-first project workspace
Comment[zh_CN]=以终端为核心的项目工作台
Exec="$escaped_binary" %f
Icon=mintool
Terminal=false
Categories=Development;
StartupWMClass=mintool
X-Mintool-Version=$version
DESKTOP
chmod 644 "$install_data/applications/mintool.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$install_data/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -f -t "$install_data/icons/hicolor" || true
fi
printf 'Mintool %s 已安装：%s\n应用菜单：%s\n' "$version" "$install_bin/mintool" "$install_data/applications/mintool.desktop"
