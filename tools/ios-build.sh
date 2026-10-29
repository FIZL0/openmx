#!/usr/bin/env bash
# Cross-build the shell for iPhone on Linux (P200, as opentennis's M91): clang + rustup's ld64.lld against
# the SDK tools/ios-sdk.py pulls out of an Xcode .xip. rustup's toolchain (with the aarch64-apple-ios std)
# sits in ~/.cargo/bin, off PATH so the gate keeps the system rustc: this script puts it first.
# Its own target dir (target/ios), so the desktop's incremental builds are not thrown away.
# Usage: tools/ios-build.sh [cargo args…]   (default: build --release -p openmx --bin openmx)
set -euo pipefail
export PATH=$HOME/.cargo/bin:$PATH
sdk=${OPENMX_IOS_SDK:-$HOME/.local/share/hst-ios/iPhoneOS.sdk}
[ -d "$sdk" ] || { echo "no SDK at $sdk: run tools/ios-sdk.py Xcode.xip" >&2; exit 1; }
rustup target list --installed | grep -qx aarch64-apple-ios || { echo "no aarch64-apple-ios std: rustup target add aarch64-apple-ios" >&2; exit 1; }
lld=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld64.lld
min=15.0
export SDKROOT=$sdk IPHONEOS_DEPLOYMENT_TARGET=$min
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$PWD/target/ios}
export CC_aarch64_apple_ios=clang CXX_aarch64_apple_ios=clang++
export CFLAGS_aarch64_apple_ios="-target arm64-apple-ios$min -isysroot $sdk"
export CXXFLAGS_aarch64_apple_ios=$CFLAGS_aarch64_apple_ios
export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER=clang
export CARGO_TARGET_AARCH64_APPLE_IOS_RUSTFLAGS="-C link-arg=-target -C link-arg=arm64-apple-ios$min -C link-arg=-isysroot -C link-arg=$sdk -C link-arg=-fuse-ld=lld -C link-arg=--ld-path=$lld"
[ $# -gt 0 ] || set -- build --release -p openmx --bin openmx
exec cargo "$1" --target aarch64-apple-ios "${@:2}"
