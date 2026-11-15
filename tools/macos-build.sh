#!/usr/bin/env bash
# P351 (opentennis's M162): cross-build the shell for Apple Silicon Macs on Linux: clang + rustup's ld64.lld against
# MacOSX.sdk (copied from a Mac, or tools/ios-sdk.py out of the same Xcode .xip as iOS; tools/ios-build.sh is the
# iPhone twin of this). rustup's toolchain (with the aarch64-apple-darwin std) sits in ~/.cargo/bin, off PATH so the
# gate keeps the system rustc: this script puts it first. Its own target dir (target/macos), so the desktop's
# incremental builds are not thrown away.
# Usage: tools/macos-build.sh [cargo args…]   (default: build --release -p openmx --bin openmx)
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
sdk=${OPENMX_MACOS_SDK:-$HOME/.local/share/hst-ios/MacOSX.sdk}
[ -d "$sdk" ] || { echo "no SDK at $sdk: copy MacOSX.sdk from a Mac, or run tools/ios-sdk.py Xcode.xip" >&2; exit 1; }
t=aarch64-apple-darwin
rustup target list --installed | grep -qx $t || { echo "no $t std: rustup target add $t" >&2; exit 1; }
lld=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld64.lld
min=11.0  # Big Sur: the first macOS on Apple Silicon, and Rust's floor for this target
export SDKROOT=$sdk MACOSX_DEPLOYMENT_TARGET=$min
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target/macos}
export CC_aarch64_apple_darwin=clang CXX_aarch64_apple_darwin=clang++
export CFLAGS_aarch64_apple_darwin="-target arm64-apple-macos$min -isysroot $sdk"
export CXXFLAGS_aarch64_apple_darwin=$CFLAGS_aarch64_apple_darwin
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=clang
export CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS="-C link-arg=-target -C link-arg=arm64-apple-macos$min -C link-arg=-isysroot -C link-arg=$sdk -C link-arg=-fuse-ld=lld -C link-arg=--ld-path=$lld"
[ $# -gt 0 ] || set -- build --release -p openmx --bin openmx
exec cargo "$1" --target $t "${@:2}"
