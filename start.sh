#!/bin/sh
set -eu

project_root="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

if command -v cargo >/dev/null 2>&1; then
    cargo_bin="$(command -v cargo)"
elif [ -n "${HOME:-}" ] && [ -x "$HOME/.cargo/bin/cargo" ]; then
    cargo_bin="${HOME}/.cargo/bin/cargo"
else
    printf '%s\n' '未找到 Rust/Cargo，请先安装 Rust 工具链：https://rustup.rs/' >&2
    exit 1
fi

# Keep native build output on the local disk when the project is on a mounted filesystem.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mintool-target}"
export CARGO_TARGET_DIR

# Cargo reuses the release build when sources have not changed.
# Keep the caller's working directory so relative project arguments work naturally.
exec "$cargo_bin" run --release --locked --manifest-path "$project_root/Cargo.toml" -- "$@"
