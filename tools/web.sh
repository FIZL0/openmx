#!/bin/sh
# Builds the browser version into web/pkg/openmx_web.wasm (N4). Serve web/ over http on localhost, or
# https anywhere else: OPFS, where the page keeps its copy of the user's disc, needs a secure context.
#
#   tools/web.sh                 # build, release
#   tools/web.sh --check         # build only, into the build directory (what crates/openmx/tests/web.rs runs)
#
# What it needs, and what it does about it (the same arrangement as crates/game-sim/tests/cross/platforms.sh,
# whose scratch toolchain it shares):
#   * the standard library for wasm32-unknown-unknown. Taken from this machine's toolchain when it has it
#     (a rustup `target add`); otherwise the matching upstream release — rustc, cargo and the two standard
#     libraries, because a distribution's rustc cannot load an upstream std — is downloaded once into
#     $OPENMX_CROSS_DIR (default $TMPDIR/openmx-cross) and used from there. Nothing is installed into the
#     system and nothing lands in the repository.
#   * nothing else: no wasm-bindgen, no wasm-opt. The module's imports and exports are plain numbers
#     (crates/openmx/src/web.rs says which), and web/worker.js instantiates it by hand.
#
# Environment:
#   OPENMX_CROSS_DIR      where the scratch toolchain lives
#   OPENMX_CROSS_OFFLINE  set to 1 to refuse to download (a missing std is then an error saying so)
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
scratch=${OPENMX_CROSS_DIR:-${TMPDIR:-/tmp}/openmx-cross}
offline=${OPENMX_CROSS_OFFLINE:-0}
target=wasm32-unknown-unknown
check=0
case ${1:-} in --check) check=1;; esac

host=$(rustc -vV | sed -n 's/^host: //p')
version=$(rustc -vV | sed -n 's/^release: //p')

has_std() { # <rustc> <target>
    d=$("$1" --print target-libdir --target "$2" 2>/dev/null || true)
    [ -n "$d" ] && [ -d "$d" ] && ls "$d"/libcore-*.rlib >/dev/null 2>&1
}

RUSTC=rustc
CARGO=cargo
if ! has_std "$RUSTC" "$target"; then
    tc=$scratch/rust-$version
    mkdir -p "$scratch"
    # Every slot's ./kit check may get here at once; one of them downloads and the rest wait for it.
    exec 9>"$scratch/fetch.lock"
    command -v flock >/dev/null 2>&1 && flock 9
    if ! has_std "$tc/bin/rustc" "$host" || ! has_std "$tc/bin/rustc" "$target"; then
        if [ "$offline" = 1 ]; then
            echo "web.sh: no standard library for $target here, and OPENMX_CROSS_OFFLINE=1" >&2
            exit 1
        fi
        base=https://static.rust-lang.org/dist
        tmp=$scratch/dl-$version
        mkdir -p "$tmp" "$tc/lib/rustlib"
        if ! has_std "$tc/bin/rustc" "$host"; then
            echo "# fetching rust $version (rustc, cargo, std for $host) into $tc" >&2
            for c in rustc cargo "rust-std"; do
                f=$c-$version-$host.tar.xz
                [ -f "$tmp/$f" ] || { curl -fsS -o "$tmp/$f.part" "$base/$f" && mv "$tmp/$f.part" "$tmp/$f"; }
                tar xf "$tmp/$f" -C "$tmp"
            done
            cp -a "$tmp/rustc-$version-$host/rustc/." "$tc/"
            cp -a "$tmp/cargo-$version-$host/cargo/." "$tc/"
            cp -a "$tmp/rust-std-$version-$host/rust-std-$host/lib/rustlib/$host" "$tc/lib/rustlib/"
        fi
        if ! has_std "$tc/bin/rustc" "$target"; then
            echo "# fetching the $target std for rust $version into $tc" >&2
            f=rust-std-$version-$target.tar.xz
            [ -f "$tmp/$f" ] || { curl -fsS -o "$tmp/$f.part" "$base/$f" && mv "$tmp/$f.part" "$tmp/$f"; }
            tar xf "$tmp/$f" -C "$tmp"
            cp -a "$tmp/rust-std-$version-$target/rust-std-$target/lib/rustlib/$target" "$tc/lib/rustlib/"
        fi
    fi
    exec 9>&-
    RUSTC=$tc/bin/rustc
    CARGO=$tc/bin/cargo
fi

# Its own build directory, beside the native one: a different compiler's artifacts in the same
# directory would rebuild both every time.
out=${CARGO_TARGET_DIR:-$repo/target}/web
RUSTC=$RUSTC CARGO_TARGET_DIR=$out "$CARGO" build -q --manifest-path "$repo/Cargo.toml" \
    -p openmx --lib --release --target "$target"
wasm=$out/$target/release/openmx_web.wasm
[ -f "$wasm" ] || { echo "web.sh: the build left no $wasm" >&2; exit 1; }
if [ "$check" = 1 ]; then
    echo "$wasm"
    exit 0
fi
mkdir -p "$repo/web/pkg"
cp "$wasm" "$repo/web/pkg/openmx_web.wasm"
echo "$repo/web/pkg/openmx_web.wasm ($(wc -c <"$wasm") bytes): serve $repo/web on localhost"
