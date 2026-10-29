#!/usr/bin/env bash
# Unsigned IPA of the shell for the user to sign (P200, as opentennis's M91): release build
# (tools/ios-build.sh), stripped, in a Payload/openMX.app with Info.plist + the P194 icon, zipped.
# Usage: tools/ios-ipa.sh [OUT.ipa]  (default target/openmx-ios.ipa)
# On the phone: the app's folder in the Files app ("On My iPhone → openMX") takes the ISO, settings.txt and mods/.
set -euo pipefail
cd "$(dirname "$0")/.."
out=$(realpath -m "${1:-target/openmx-ios.ipa}")
build=$(git rev-list --count HEAD)
OPENMX_BUILD=$build OPENMX_COMMIT=$(git rev-parse --short HEAD) tools/ios-build.sh build --release -p openmx --bin openmx
target=${CARGO_TARGET_DIR:-$PWD/target/ios}/aarch64-apple-ios
stage=$target/ipa
app=$stage/Payload/openMX.app
rm -rf "$stage"
mkdir -p "$app"
llvm-strip -o "$app/openmx" "$target/release/openmx"
cp web/icon-180.png "$app/AppIcon60x60@3x.png"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)  # [workspace.package]
cat > "$app/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key><string>openmx</string>
	<key>CFBundleIdentifier</key><string>io.github.fizl0.openmx</string>
	<key>CFBundleName</key><string>openMX</string>
	<key>CFBundleDisplayName</key><string>openMX</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$build</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleSupportedPlatforms</key><array><string>iPhoneOS</string></array>
	<key>DTPlatformName</key><string>iphoneos</string>
	<key>MinimumOSVersion</key><string>15.0</string>
	<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
	<key>UIRequiredDeviceCapabilities</key><array><string>arm64</string><string>metal</string></array>
	<key>CFBundleIcons</key><dict><key>CFBundlePrimaryIcon</key><dict>
		<key>CFBundleIconFiles</key><array><string>AppIcon60x60</string></array>
	</dict></dict>
	<key>UIApplicationSceneManifest</key><dict>
		<key>UIApplicationSupportsMultipleScenes</key><false/>
		<key>UISceneConfigurations</key><dict>
			<key>UIWindowSceneSessionRoleApplication</key><array><dict>
				<key>UISceneConfigurationName</key><string>Default</string>
			</dict></array>
		</dict>
	</dict>
	<key>UILaunchScreen</key><dict/>
	<key>UIRequiresFullScreen</key><true/>
	<key>UIStatusBarHidden</key><true/>
	<!-- M144: portrait too; with rotation lock on the game holds landscape, island on the left (B694, ios_gpu.rs) -->
	<key>UISupportedInterfaceOrientations</key><array>
		<string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string>
		<string>UIInterfaceOrientationPortrait</string>
	</array>
	<key>UISupportedInterfaceOrientations~ipad</key><array>
		<string>UIInterfaceOrientationLandscapeLeft</string><string>UIInterfaceOrientationLandscapeRight</string>
		<string>UIInterfaceOrientationPortrait</string><string>UIInterfaceOrientationPortraitUpsideDown</string>
	</array>
	<key>CADisableMinimumFrameDurationOnPhone</key><true/>
	<key>UIFileSharingEnabled</key><true/>
	<key>LSSupportsOpeningDocumentsInPlace</key><true/>
	<key>NSLocalNetworkUsageDescription</key><string>Finds and joins rides on your local network.</string>
</dict>
</plist>
EOF
rm -f "$out"
(cd "$stage" && zip -qr -X "$out" Payload)
echo "$out ($(du -h "$out" | cut -f1))"
