#!/bin/sh
# Run the simulation's tests on an architecture that is not this machine's.
#
#   crates/game-sim/tests/cross/platforms.sh              # every target it can run
#   crates/game-sim/tests/cross/platforms.sh --list       # what it would run, and with what
#   crates/game-sim/tests/cross/platforms.sh aarch64-unknown-linux-musl
#
# Why this exists: `tests/platforms.rs` pins the state hash, the fixture vectors and the game's libm as
# literal bit patterns, so "the same fixture hashes identically on x86-64 and on aarch64" is proved by
# running *that same test binary*, compiled for the other architecture, against the same literals.
# One command, so the proof is repeatable by whoever doubts it rather than a line in a journal entry.
#
# What it needs, and what it does about it:
#   * the standard library for the target. Taken from this machine's toolchain when it has it (a rustup
#     `+target add`); otherwise the matching upstream release is downloaded once into a scratch
#     directory ($OPENMX_CROSS_DIR, default $TMPDIR/openmx-cross) and used from there. Nothing is ever
#     installed into the system, and nothing lands in the repository.
#   * a way to execute the result: qemu-aarch64[-static] for aarch64, node (>= 20, for `node:wasi`) for
#     wasm32. A target whose runner is missing is reported as skipped, not as a pass.
#   * a linker for musl: rust-lld, which ships with the toolchain, plus the target's self-contained
#     libc.a. So no cross-gcc and no cross-glibc are needed.
#
# Environment:
#   OPENMX_CROSS_DIR      where the scratch toolchain and build artifacts live
#   OPENMX_CROSS_OFFLINE  set to 1 to refuse to download (a missing std is then a skip)
#   OPENMX_CROSS_TESTS    which test targets to run (default: the simulation's own)
#
# Exit status: 0 when every target it ran passed and at least one of them was a different architecture
# from this machine's; non-zero when a target failed or nothing could be proved.
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../../../.." && pwd)
scratch=${OPENMX_CROSS_DIR:-${TMPDIR:-/tmp}/openmx-cross}
offline=${OPENMX_CROSS_OFFLINE:-0}
tests=${OPENMX_CROSS_TESTS:-"--test platforms --test hash --test replay --test rng --test snapshot"}

host=$(rustc -vV | sed -n 's/^host: //p')
version=$(rustc -vV | sed -n 's/^release: //p')
host_arch=${host%%-*}

# The targets worth running: a different byte order of the registers (aarch64) and a different pointer
# width (wasm32, where `usize` is 32 bits and a length that leaked into the hash would show up).
default_targets="aarch64-unknown-linux-musl wasm32-wasip1"
case $host_arch in aarch64) default_targets="x86_64-unknown-linux-musl wasm32-wasip1";; esac

list_only=0
case ${1:-} in --list) list_only=1; shift;; esac
targets=${*:-$default_targets}

# --- the toolchain ---------------------------------------------------------------------------------

std_dir() { # <rustc> <target>
    "$1" --print target-libdir --target "$2" 2>/dev/null || true
}
has_std() { # <rustc> <target>
    d=$(std_dir "$1" "$2")
    [ -n "$d" ] && [ -d "$d" ] && ls "$d"/libcore-*.rlib >/dev/null 2>&1
}

# This machine's rustc first: a rustup user with the target installed needs no download. Note that a
# distribution's rustc cannot load an upstream std (the version string is part of the crate hash), which
# is why the fallback below brings its own rustc and cargo rather than only the standard libraries.
RUSTC=rustc
CARGO=cargo
need_download=0
for t in $targets; do
    has_std "$RUSTC" "$t" || need_download=1
done

if [ "$need_download" = 1 ]; then
    tc=$scratch/rust-$version
    if ! has_std "$tc/bin/rustc" "$host"; then
        if [ "$offline" = 1 ]; then
            echo "skipped: no std for [$targets] and OPENMX_CROSS_OFFLINE=1" >&2
            exit 1
        fi
        echo "# fetching rust $version (rustc, cargo, std for $host and the targets) into $tc"
        tmp=$scratch/dl-$version
        mkdir -p "$tmp" "$tc"
        base=https://static.rust-lang.org/dist
        for c in rustc cargo rust-std; do
            f=$c-$version-$host.tar.xz
            [ -f "$tmp/$f" ] || curl -fsS -o "$tmp/$f" "$base/$f"
            tar xf "$tmp/$f" -C "$tmp"
        done
        cp -a "$tmp/rustc-$version-$host/rustc/." "$tc/"
        cp -a "$tmp/cargo-$version-$host/cargo/." "$tc/"
        cp -a "$tmp/rust-std-$version-$host/rust-std-$host/lib/rustlib/$host" "$tc/lib/rustlib/"
    fi
    for t in $targets; do
        if ! has_std "$tc/bin/rustc" "$t"; then
            f=rust-std-$version-$t.tar.xz
            tmp=$scratch/dl-$version
            mkdir -p "$tmp"
            [ -f "$tmp/$f" ] || curl -fsS -o "$tmp/$f" "https://static.rust-lang.org/dist/$f"
            tar xf "$tmp/$f" -C "$tmp"
            cp -a "$tmp/rust-std-$version-$t/rust-std-$t/lib/rustlib/$t" "$tc/lib/rustlib/"
        fi
    done
    RUSTC=$tc/bin/rustc
    CARGO=$tc/bin/cargo
fi

sysroot=$("$RUSTC" --print sysroot)
lld=$sysroot/lib/rustlib/$host/bin/rust-lld

# --- how each target is built and run --------------------------------------------------------------

runner_for() { # <target> -> the command, or empty with a reason on stderr
    case $1 in
    aarch64-*)
        for q in qemu-aarch64-static qemu-aarch64; do
            command -v "$q" >/dev/null 2>&1 && { echo "$q"; return 0; }
        done
        echo "no qemu-aarch64" >&2; return 1;;
    wasm32-*)
        command -v node >/dev/null 2>&1 || { echo "no node" >&2; return 1; }
        echo "node --no-warnings $here/wasi-run.js"; return 0;;
    "$host")
        echo ""; return 0;;
    x86_64-unknown-linux-musl)
        [ "$host_arch" = x86_64 ] && { echo ""; return 0; }
        for q in qemu-x86_64-static qemu-x86_64; do
            command -v "$q" >/dev/null 2>&1 && { echo "$q"; return 0; }
        done
        echo "no qemu-x86_64" >&2; return 1;;
    *)
        echo "no runner known for $1" >&2; return 1;;
    esac
}

env_var() { # <target> <suffix> -> CARGO_TARGET_<TRIPLE>_<SUFFIX>
    echo "CARGO_TARGET_$(echo "$1" | tr 'a-z-' 'A-Z_')_$2"
}

ran=0
failed=0
for t in $targets; do
    if ! has_std "$RUSTC" "$t"; then
        echo "$t skipped: no standard library for it"
        continue
    fi
    why=$scratch/why.$$
    mkdir -p "$scratch"
    if ! runner=$(runner_for "$t" 2>"$why"); then
        echo "$t skipped: $(cat "$why")"
        rm -f "$why"
        continue
    fi
    rm -f "$why"
    if [ "$list_only" = 1 ]; then
        echo "$t would run under: ${runner:-this machine}"
        ran=$((ran + 1))
        continue
    fi

    runner_var=$(env_var "$t" RUNNER)
    rustflags_var=$(env_var "$t" RUSTFLAGS)
    linker_var=$(env_var "$t" LINKER)

    # The arithmetic model's own tests as well as the simulation's: `ps2-float` is where a host float
    # would do the damage, and its tests are the ones that pin the EE's rounding.
    ok=1
    for spec in "-p ps2-float --test fpu --test vu --test libm" "-p game-sim $tests"; do
        (
            export "$runner_var=$runner"
            export CARGO_TARGET_DIR="$scratch/target" RUSTC="$RUSTC"
            # musl links statically through rust-lld and the target's own libc.a, so no cross-gcc and
            # no cross-glibc are needed. wasm32 already links with rust-lld by itself.
            case $t in
            *-linux-musl)
                export "$rustflags_var=-Clinker-flavor=ld.lld -Clink-self-contained=yes"
                export "$linker_var=$lld";;
            esac
            # shellcheck disable=SC2086  # $spec and $tests are lists of arguments on purpose
            "$CARGO" test -q --manifest-path "$repo/Cargo.toml" --target "$t" $spec
        ) || ok=0
    done
    if [ "$ok" = 1 ]; then
        echo "$t ok"
    else
        echo "$t FAILED"
        failed=$((failed + 1))
    fi
    ran=$((ran + 1))
done

[ "$failed" = 0 ] || exit 1
[ "$ran" -gt 0 ] || { echo "nothing ran: no target could be built and executed here" >&2; exit 1; }
echo "$ran target(s), host is $host"
