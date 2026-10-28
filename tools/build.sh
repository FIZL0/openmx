#!/bin/bash
# tools/build.sh [linux|windows|all] — the optimised builds players download, into dist/ (as opentennis's tools/build.sh):
#   dist/linux/         openmx, run.sh, kit.toml, README.txt: unpack, put the disc's path in kit.toml, ./run.sh
#   dist/windows/       openmx.exe — not yet: the shell's pads (/dev/input/js*), sound (pw-cat) and software window (X11)
#                       are Linux-only until the Windows port lands (plan/TODO.md), so `windows` says so and stops
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

# what goes beside the binary: the run script (the shell finds kit.toml from the working directory), a kit.toml
# naming the disc, and how to start it. $1 is the folder.
extras() {
    printf '#!/bin/sh\n# openMX looks for kit.toml in the folder it is started from: start it from this one\ncd "$(dirname "$0")" && exec ./openmx "$@"\n' >"$1/run.sh"
    chmod +x "$1/run.sh"
    cat >"$1/kit.toml" <<'EOF'
# Where your own dump of MX Unleashed (USA, SLUS-20767) is: a path relative to this folder, or an absolute one.
[game]
serial = "SLUS-20767"
iso    = "MX Unleashed (USA).iso"
EOF
    cat >"$1/README.txt" <<'EOF'
openMX — MX Unleashed (PS2) rewritten. https://openmx.dev/

No game data is included. You need your own disc image of MX Unleashed (USA, SLUS-20767).

1. Put your .iso in this folder as "MX Unleashed (USA).iso", or write its path into kit.toml.
2. Run ./run.sh (or ./run.sh --help for the options).

Settings, controls and the mods/ folder live beside the disc image.
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
    echo "no Windows build yet: the shell's pads, sound and software window are Linux-only (plan/TODO.md, the Windows port)" >&2
    exit 1
}

case $what in
    linux) linux ;;
    windows) windows ;;
    all) linux ;;  # every platform that builds today; add windows here when it does
    *) echo "usage: tools/build.sh [linux|windows|all]" >&2; exit 2 ;;
esac
