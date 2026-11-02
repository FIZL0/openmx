#!/bin/sh
# The Firefox the web tests drive (P245): Mozilla's own release build, fetched once and checked against its
# SHA-256, unpacked under $OPENMX_FIREFOX_DIR (default ${XDG_CACHE_HOME:-~/.cache}/openmx/firefox-<version>), and
# its binary's path printed. Nothing is installed into the system; a firefox on PATH is not used, since a distro's
# build may lack the test pref the gamepad test needs (dom.gamepad.test.enabled) or be a different version.
# Offline with nothing cached it fails and says so.
set -eu
version=159.0
sum=54536fcd120af484d4c56272a5a1cf4af57b9c713d1a3b1e775da192dd541fcd
dir=${OPENMX_FIREFOX_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/openmx/firefox-$version}
if [ ! -x "$dir/firefox/firefox" ]; then
    mkdir -p "$dir"
    tar="$dir/firefox-$version.tar.xz"
    if [ ! -f "$tar" ]; then
        curl -fsSL -o "$tar.part" "https://ftp.mozilla.org/pub/firefox/releases/$version/linux-x86_64/en-US/firefox-$version.tar.xz" \
            || { echo "tools/firefox.sh: could not download Firefox $version" >&2; rm -f "$tar.part"; exit 1; }
        echo "$sum  $tar.part" | sha256sum -c --quiet - || { echo "tools/firefox.sh: Firefox $version's checksum is wrong" >&2; rm -f "$tar.part"; exit 1; }
        mv "$tar.part" "$tar"
    fi
    rm -rf "$dir/unpack"
    mkdir "$dir/unpack"
    tar -xJf "$tar" -C "$dir/unpack"
    mv "$dir/unpack/firefox" "$dir/firefox"
    rmdir "$dir/unpack"
    rm -f "$tar"
fi
echo "$dir/firefox/firefox"
