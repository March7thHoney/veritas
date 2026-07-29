#!/usr/bin/env bash
set -euo pipefail

TOOLCHAIN="nightly-2025-05-17"
TARGET="x86_64-pc-windows-msvc"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

command -v rustup >/dev/null || { echo "rustup is required" >&2; exit 1; }
command -v cargo-xwin >/dev/null || { echo "cargo-xwin is required" >&2; exit 1; }
rustup which --toolchain "$TOOLCHAIN" rustc >/dev/null 2>&1 || {
    echo "missing toolchain: $TOOLCHAIN" >&2
    exit 1
}
rustup target list --installed --toolchain "$TOOLCHAIN" | grep -qx "$TARGET" || {
    echo "missing target: $TARGET for $TOOLCHAIN" >&2
    exit 1
}

LLVM_RC_BIN="${LLVM_RC:-$(command -v llvm-rc || true)}"
if [[ -z "$LLVM_RC_BIN" ]] && command -v brew >/dev/null; then
    LLVM_PREFIX="$(brew --prefix llvm 2>/dev/null || true)"
    [[ -x "$LLVM_PREFIX/bin/llvm-rc" ]] && LLVM_RC_BIN="$LLVM_PREFIX/bin/llvm-rc"
fi
[[ -x "$LLVM_RC_BIN" ]] || { echo "llvm-rc is required" >&2; exit 1; }

export RUSTC="$(rustup which --toolchain "$TOOLCHAIN" rustc)"
export RUSTDOC="$(rustup which --toolchain "$TOOLCHAIN" rustdoc)"
export LLVM_RC="$LLVM_RC_BIN"
export CFLAGS="${CFLAGS:+$CFLAGS }/FIstring.h"

cd "$PROJECT_DIR"
rustup run "$TOOLCHAIN" cargo xwin build --release --locked --target "$TARGET"
