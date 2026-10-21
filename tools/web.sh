#!/bin/sh
# Builds the browser version into web/pkg (N4): openmx_web_bg.wasm and wasm-bindgen's openmx_web.js.
# Serve web/ over http on localhost, or https anywhere else: OPFS, where the page keeps its copy of
# the user's disc, needs a secure context.
#
#   tools/web.sh                 # build, release
#   tools/web.sh --check         # build only, into the build directory's pkg/, and print where
#                                # (what crates/openmx/tests/web.rs and tests/netplay/browser.rs run)
#
# What it needs, and what it does about it (the same arrangement as crates/game-sim/tests/cross/platforms.sh,
# whose scratch toolchain it shares):
#   * the standard library for wasm32-unknown-unknown. Taken from this machine's toolchain when it has it
#     (a rustup `target add`); otherwise the matching upstream release — rustc, cargo and the two standard
#     libraries, because a distribution's rustc cannot load an upstream std — is downloaded once into
#     $OPENMX_CROSS_DIR (default $TMPDIR/openmx-cross) and used from there. Nothing is installed into the
#     system and nothing lands in the repository.
#   * wasm-bindgen-cli at the version Cargo.lock has of wasm-bindgen (the two must match exactly), as
#     opentennis's web.sh does: ggrs and iroh take the page's clock, random numbers and WebSocket
#     through it. Taken from PATH when it is that version; otherwise built once with this machine's
#     cargo into $OPENMX_CROSS_DIR/wasm-bindgen-<version>. No wasm-opt.
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

wbv=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' "$repo/Cargo.lock")
[ -n "$wbv" ] || { echo "web.sh: Cargo.lock has no wasm-bindgen" >&2; exit 1; }
wb=$(command -v wasm-bindgen || true)
if [ -z "$wb" ] || [ "$("$wb" --version 2>/dev/null)" != "wasm-bindgen $wbv" ]; then
    wb=$scratch/wasm-bindgen-$wbv/bin/wasm-bindgen
    mkdir -p "$scratch"
    exec 9>"$scratch/fetch.lock"
    command -v flock >/dev/null 2>&1 && flock 9
    if [ ! -x "$wb" ]; then
        if [ "$offline" = 1 ]; then
            echo "web.sh: no wasm-bindgen $wbv here, and OPENMX_CROSS_OFFLINE=1" >&2
            exit 1
        fi
        echo "# building wasm-bindgen-cli $wbv into $scratch/wasm-bindgen-$wbv" >&2
        cargo install -q --locked wasm-bindgen-cli --version "$wbv" --root "$scratch/wasm-bindgen-$wbv" >&2
    fi
    exec 9>&-
fi

# One bindgen at a time into the build directory (every test that needs the module runs this), and
# the result moved into place whole.
exec 8>"$out/pkg.lock"
command -v flock >/dev/null 2>&1 && flock 8
rm -rf "$out/pkg.new"
"$wb" --target web --no-typescript --out-dir "$out/pkg.new" "$wasm"
rm -rf "$out/pkg"
mv "$out/pkg.new" "$out/pkg"
exec 8>&-
if [ "$check" = 1 ]; then
    echo "$out/pkg"
    exit 0
fi
mkdir -p "$repo/web/pkg"
cp "$out/pkg/openmx_web.js" "$out/pkg/openmx_web_bg.wasm" "$repo/web/pkg/"
echo "$repo/web/pkg ($(wc -c <"$out/pkg/openmx_web_bg.wasm") bytes of module): serve $repo/web on localhost"
