#!/bin/bash
# tools/build.sh [linux|windows|all] — the optimised builds players download, into dist/ (as opentennis's tools/build.sh):
#   dist/linux/         openmx, openmx.png (the icon, for a launcher or Steam shortcut), README.txt: unpack,
#                       ./openmx "your disc.iso" (or drop the .iso on it) the first time, ./openmx after that;
#                       settings, controls and mods/ in ~/.local/share/openmx (P190)
#   dist/windows/       openmx.exe (P192), its icon built in: drop the .iso on it the first time; settings, controls
#                       and mods\ in %APPDATA%\openmx. Cross-compiled with MinGW, once (Arch):
#                         sudo pacman -S mingw-w64-gcc && rustup target add x86_64-pc-windows-gnu
# No game data goes into dist/. OPENMX_COMMIT / OPENMX_VERSION in the environment are passed into the build
# (tools/release.sh and the Actions jobs set them).
#
# Linux is built inside Valve's Steam Runtime "sniper" SDK container (glibc 2.31, Debian 11), so it runs on every
# SteamOS 3.x / Steam Deck and any desktop distribution of the last five years. One-time setup:
#   sudo pacman -S docker && sudo systemctl enable --now docker && sudo usermod -aG docker $USER   # then log in again
# The container's Rust toolchain and cargo cache live in ~/.cache/openmx-steamrt; its build in target/steamrt.
set -e
cd "$(dirname "$0")/.."
what=${1:-linux}

sdk=registry.gitlab.steamos.cloud/steamrt/sniper/sdk:latest

if [ -z "$OPENMX_COMMIT" ]; then
    OPENMX_COMMIT=$(git rev-parse HEAD | cut -c1-7)
    [ -z "$(git status --porcelain --untracked-files=no)" ] || OPENMX_COMMIT=$OPENMX_COMMIT+
fi
export OPENMX_COMMIT

# what goes beside the binary: how to start it. With no kit.toml the shell takes its disc from its first argument
# and remembers it, its settings in the per-user data folder (P190). $1 is the folder.
extras() {
    cp web/icon-512.png "$1/openmx.png"
    cat >"$1/README.txt" <<'EOF'
openMX — MX Unleashed (PS2) rewritten. https://openmx.dev/

No game data is included. You need your own disc image of MX Unleashed (USA, SLUS-20767).

1. The first time, start it with your disc: ./openmx "/path/to/MX Unleashed (USA).iso",
   or drop the .iso on openmx in your file manager. Started with nothing, it asks for the disc
   (a file picker when zenity or kdialog is installed, else in the terminal).
2. After that, ./openmx: it remembers the disc. Start it with another .iso to change it.

Settings, controls and the mods/ folder live in ~/.local/share/openmx ($XDG_DATA_HOME/openmx),
beside disc.txt, the disc it remembers. ./openmx --where says which disc and which folder;
./openmx --help lists the options.
EOF
}

linux() {
    if ! docker info >/dev/null 2>&1; then
        echo "linux build needs docker (once): sudo pacman -S docker; sudo systemctl enable --now docker; sudo usermod -aG docker \$USER; log in again" >&2
        exit 1
    fi
    mkdir -p ~/.cache/openmx-steamrt
    docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/src" -v ~/.cache/openmx-steamrt:/cache -w /src \
        -e OPENMX_VERSION -e OPENMX_COMMIT -e HOME=/cache -e CARGO_HOME=/cache/cargo -e RUSTUP_HOME=/cache/rustup -e CARGO_TARGET_DIR=/src/target/steamrt \
        $sdk sh -ec '
            [ -x /cache/cargo/bin/cargo ] || curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --no-modify-path
            /cache/cargo/bin/rustup update stable >/dev/null 2>&1
            /cache/cargo/bin/cargo build --locked --release -p openmx --bin openmx'
    rm -rf dist/linux && mkdir -p dist/linux  # the tarball is the whole folder: nothing stale in it
    strip -o dist/linux/openmx target/steamrt/release/openmx
    extras dist/linux
    local need glibc=2.31
    need=$(objdump -T dist/linux/openmx | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
    echo "dist/linux/openmx needs $need (sniper has GLIBC_$glibc)"
    if [ "$(printf '%s\n' "${need#GLIBC_}" $glibc | sort -V | tail -1)" != $glibc ]; then
        echo "dist/linux/openmx needs $need, newer than GLIBC_$glibc: won't start on SteamOS" >&2
        exit 1
    fi
}

windows() {
    local t=x86_64-pc-windows-gnu
    if ! command -v x86_64-w64-mingw32-gcc >/dev/null || ! rustc --print target-libdir --target $t 2>/dev/null | xargs test -d; then
        echo "windows build needs (once): sudo pacman -S mingw-w64-gcc; rustup target add $t" >&2
        exit 1
    fi
    # crt-static: no MinGW runtime DLLs to ship beside the .exe
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc RUSTFLAGS="-C target-feature=+crt-static" \
        cargo build --locked --release -p openmx --bin openmx --target $t
    rm -rf dist/windows && mkdir -p dist/windows
    x86_64-w64-mingw32-strip -o dist/windows/openmx.exe target/$t/release/openmx.exe
    echo "dist/windows/openmx.exe"
}

case $what in
    linux) linux ;;
    windows) windows ;;
    all) linux; windows ;;
    *) echo "usage: tools/build.sh [linux|windows|all]" >&2; exit 2 ;;
esac
