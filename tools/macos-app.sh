#!/usr/bin/env bash
# P351 (opentennis's M162): openMX.app for Apple Silicon Macs, in a .dmg that opens on the app beside an Applications
# link to drag it onto: release build (tools/macos-build.sh), stripped, Info.plist + icon, ad-hoc signed with rcodesign
# (Apple Silicon runs nothing unsigned; no Apple account, so no notarization: the player opens it once with
# right-click → Open, or on macOS 15 and later System Settings → Privacy & Security → Open Anyway).
# Like Linux, the first start asks for the ISO (player.rs, AppleScript's file dialog) and keeps settings, controls and
# mods/ in ~/Library/Application Support/openmx.
# The app is a player build, so on the native sim (P262, as tools/build.sh's); --exact-sim builds the PS2-arithmetic one.
# Both take `ship` (P312, opentennis's B492): the developers' OPENMX_* variables are dropped as it starts.
# Usage: tools/macos-app.sh [OUT.dmg] [--exact-sim]   (default target/openmx-macos-arm64.dmg)
# Needs (once): the SDK (tools/macos-build.sh), cargo install apple-codesign (rcodesign), ImageMagick (the icon), and
# the .dmg makers in ~/.local/bin, built from source as Bitcoin Core's Linux-made .dmg does it: xorrisofs (GNU
# xorriso: ./configure && make, xorriso/xorriso → xorrisofs) and dmg (github.com/fanquake/libdmg-hfsplus: cmake).
set -euo pipefail
cd "$(dirname "$0")/.."
features="--features ship,native-sim" out=target/openmx-macos-arm64.dmg
for a; do
    case $a in
        --exact-sim) features="--features ship" ;;
        *) out=$a ;;
    esac
done
out=$(realpath -m "$out")
PATH=$PATH:$HOME/.cargo/bin:$HOME/.local/bin  # where cargo install puts rcodesign; the .dmg makers
command -v rcodesign >/dev/null || { echo "needs (once): cargo install apple-codesign   (rcodesign, the ad-hoc signer)" >&2; exit 1; }
for t in xorrisofs dmg magick; do command -v $t >/dev/null || { echo "needs (once): $t (see the top of this script)" >&2; exit 1; }; done
build=$(git rev-list --count HEAD)
# the caller's commit stamp when it has one (tools/release.sh), so the online build id is every platform's
OPENMX_BUILD=$build OPENMX_COMMIT=${OPENMX_COMMIT:-$(git rev-parse --short HEAD)} tools/macos-build.sh build --release -p openmx --bin openmx $features
target=${CARGO_TARGET_DIR:-$PWD/target/macos}/aarch64-apple-darwin
stage=$target/app
app=$stage/openMX.app
rm -rf "$stage"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
llvm-strip -o "$app/Contents/MacOS/openmx" "$target/release/openmx"
# the icon: an .icns of PNG entries (ic07 128, ic08 256, ic09 512, ic10 1024 px)
icons=$stage/icons && mkdir -p "$icons"
for px in 128 256 512 1024; do magick web/icon-1024.png -resize ${px}x$px "$icons/$px.png"; done
python3 -I - "$icons" "$app/Contents/Resources/AppIcon.icns" <<'PY'
import struct, sys
d, out = sys.argv[1:]
body = b"".join(t + struct.pack(">I", 8 + len(png)) + png for t, png in
                ((t.encode(), open(f"{d}/{px}.png", "rb").read()) for t, px in (("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024))))
open(out, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
PY
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)  # [workspace.package]
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key><string>openmx</string>
	<key>CFBundleIdentifier</key><string>io.github.fizl0.openmx</string>
	<key>CFBundleName</key><string>openMX</string>
	<key>CFBundleDisplayName</key><string>openMX</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleIconFile</key><string>AppIcon</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$build</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
	<key>LSMinimumSystemVersion</key><string>11.0</string>
	<key>LSArchitecturePriority</key><array><string>arm64</string></array>
	<key>LSApplicationCategoryType</key><string>public.app-category.racing-games</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
	<key>NSLocalNetworkUsageDescription</key><string>Finds and joins rides on your local network.</string>
</dict>
</plist>
PLIST
rcodesign sign "$app" >/dev/null  # no certificate given: an ad-hoc signature (binds Info.plist and the binary)
# the .dmg: an ISO 9660 + Rock Ridge image (keeps the symlink and modes) of the app and an Applications link, compressed
vol=$stage/dmg && mkdir -p "$vol" && mv "$app" "$vol/" && ln -s /Applications "$vol/Applications"
xorrisofs -quiet -D -l -V openMX -no-pad -r -dir-mode 0755 -o "$stage/raw.iso" "$vol"
rm -f "$out"
dmg "$stage/raw.iso" "$out" >/dev/null
echo "$out ($(du -h "$out" | cut -f1))"
