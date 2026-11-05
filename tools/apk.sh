#!/usr/bin/env bash
# tools/apk.sh [--install] [--exact-sim] — the Android app (P308), as opentennis's tools/apk.sh builds its own:
# dist/openmx.apk, a NativeActivity with no gradle. The shell is the `openmx` example built as a cdylib for each ABI
# with the NDK's clang (`android` and `ship` features, and the player builds' native sim unless --exact-sim, as
# tools/build.sh); the activity's few lines of Java (crates/openmx/android/) go through javac and R8; aapt2, zipalign
# and apksigner with ~/.android/debug.keystore do the rest. --install puts it on the phone (or emulator) adb sees.
# OPENMX_ABIS="arm64-v8a x86_64" adds the emulator's ABI (default: phones only, arm64-v8a). OPENMX_VERSION=v1.2.3
# (tools/release.sh) names the version, and its versionCode keeps each release an upgrade.
# Needs ~/Android/Sdk (build-tools, platforms/android-36, ndk), a JDK 17 or newer, rsvg-convert and magick, and
# `rustup target add aarch64-linux-android x86_64-linux-android`.
# The disc: picked on the phone on first start, or adb push "<ISO>" /sdcard/Android/data/io.github.fizl0.openmx/files/
# No game data goes into the .apk.
set -euo pipefail
cd "$(dirname "$0")/.."
install= features="--features android,ship,native-sim"
for a in "$@"; do
    case $a in
        --install) install=1 ;;
        --exact-sim) features="--features android,ship" ;;
        *) echo "usage: tools/apk.sh [--install] [--exact-sim]" >&2; exit 2 ;;
    esac
done
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
[ -d "$SDK" ] || { echo "no Android SDK in $SDK (set ANDROID_HOME)" >&2; exit 1; }
NDK=${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}
BT=$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)
JAR=$(ls -d "$SDK"/platforms/android-* | sort -V | tail -1)/android.jar
T=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
# javac --release 17 and R8 want a JDK 17+; the one on PATH may be older
jdk=none
for j in ${JAVA_HOME:-} "$HOME"/.local/share/mise/installs/java/temurin-2[1-9]* /usr/lib/jvm/java-2[1-9]-openjdk /usr/lib/jvm/java-17-openjdk ""; do
    b=${j:+$j/bin/}
    [ -x "${b}javac" ] || command -v "${b}javac" >/dev/null || continue
    v=$("${b}java" -version 2>&1 | awk -F'"' '/version/{split($2, p, "."); print p[1]; exit}')
    [ "${v:-0}" -ge 17 ] 2>/dev/null && { jdk=$b; break; }
done
[ "$jdk" != none ] || { echo "tools/apk.sh needs a JDK 17 or newer (set JAVA_HOME)" >&2; exit 1; }
for t in rsvg-convert magick; do command -v $t >/dev/null || { echo "$t missing: sudo pacman -S librsvg imagemagick" >&2; exit 1; }; done
if [ -z "${OPENMX_COMMIT:-}" ]; then
    OPENMX_COMMIT=$(git rev-parse HEAD | cut -c1-7)
    [ -z "$(git status --porcelain --untracked-files=no)" ] || OPENMX_COMMIT=$OPENMX_COMMIT+
fi
export OPENMX_COMMIT
out=target/apk; rm -rf "$out/lib"; mkdir -p "$out" dist
for abi in ${OPENMX_ABIS:-arm64-v8a}; do
    case $abi in arm64-v8a) t=aarch64-linux-android ;; x86_64) t=x86_64-linux-android ;; *) echo "unknown ABI $abi" >&2; exit 2 ;; esac
    T_=${t//-/_}; cc=$T/${t}26-clang
    # the .so out of its own target folder: the desktop's build keeps its cache
    env "CARGO_TARGET_${T_^^}_LINKER=$cc" "CC_$T_=$cc" "CXX_$T_=$cc++" "AR_$T_=$T/llvm-ar" CARGO_TARGET_DIR=target/android \
        cargo build --locked --release -p openmx --example openmx $features --target $t
    mkdir -p "$out/lib/$abi"
    "$T/llvm-strip" -o "$out/lib/$abi/libopenmx.so" target/android/$t/release/examples/libopenmx.so
done
# Launcher icon: the app's (web/icon.svg, P225: the rider on the dirt bike, cream on the dark red). Its 64 frame maps
# onto 58dp of the adaptive icon's 108, so the wheels stay inside the 66dp the round masks keep; the legacy icons are
# the whole rounded frame at 48dp, and a round one the rider on a disc of the red.
bg='#8e0f17'; res=$out/res; rm -rf "$res"; mkdir -p "$res/mipmap-anydpi-v26" "$res/values"
sed '/<rect /d' web/icon.svg | rsvg-convert -w 1024 >"$out/badge.png"
rsvg-convert -w 1024 web/icon.svg >"$out/whole.png"
# Themed (Android 13+) monochrome layer: the launcher tints its alpha, which is the rider already (the frame dropped).
cp "$out/badge.png" "$out/mono.png"
dp() { awk "BEGIN{print int($1*$s+.5)}"; }
for d in mdpi:1 hdpi:1.5 xhdpi:2 xxhdpi:3 xxxhdpi:4; do
    s=${d#*:}; dir=$res/mipmap-${d%:*}; mkdir -p "$dir"; a=$(dp 108); l=$(dp 48); r=$(awk "BEGIN{print 24*$s}")
    magick -size ${a}x$a xc:none \( "$out/badge.png" -resize "$(dp 58)x" \) -gravity center -composite "$dir/ic_launcher_foreground.png"
    magick -size ${a}x$a xc:none \( "$out/mono.png" -resize "$(dp 58)x" \) -gravity center -composite "$dir/ic_launcher_monochrome.png"
    magick "$out/whole.png" -resize "${l}x$l" "$dir/ic_launcher.png"
    magick -size ${l}x$l xc:none -fill "$bg" -draw "circle $r,$r $r,0" \
        \( "$out/badge.png" -resize "$(dp 40)x" \) -gravity center -composite "$dir/ic_launcher_round.png"
done
echo "<resources><color name=\"ic_launcher_background\">$bg</color></resources>" >"$res/values/ic_launcher.xml"
for n in ic_launcher ic_launcher_round; do
    echo '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
  <background android:drawable="@color/ic_launcher_background"/><foreground android:drawable="@mipmap/ic_launcher_foreground"/>
  <monochrome android:drawable="@mipmap/ic_launcher_monochrome"/>
</adaptive-icon>' >"$res/mipmap-anydpi-v26/$n.xml"
done
# opentennis's B418: rustls-platform-verifier's Java side (the store's, the updater's and the short codes' HTTPS check
# certificates through Android's own verifier, openmx-android's init_tls): its AAR, the version Cargo.lock's
# rustls-platform-verifier-android names, and the Kotlin stdlib its classes call. Downloaded once into
# ~/.cache/openmx-apk, checked against pinned hashes.
rpv=$(awk '/^name = "rustls-platform-verifier-android"$/{getline; gsub(/[^0-9.]/,""); print}' Cargo.lock)
[ "$rpv" = 0.2.0 ] || { echo "Cargo.lock has rustls-platform-verifier-android $rpv: update its AAR's version and hash here" >&2; exit 2; }
cache=${XDG_CACHE_HOME:-$HOME/.cache}/openmx-apk; mkdir -p "$cache"
fetch() { # url sha256 → its path in the cache
    local f=$cache/${1##*/}
    if ! echo "$2  $f" | sha256sum -c --status 2>/dev/null; then
        curl -sfL -o "$f.part" "$1" || { echo "can't download $1" >&2; exit 2; }
        echo "$2  $f.part" | sha256sum -c --status || { echo "$1: wrong sha256" >&2; exit 2; }
        mv "$f.part" "$f"
    fi
    echo "$f"
}
aar=$(fetch https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/org/rustls/rustls-platform-verifier/$rpv/rustls-platform-verifier-$rpv.aar \
    aa021794230fbc2f0be355999e2cf67398dd563de066a2e17001ffbd0b69101b)
kotlin=$(fetch https://repo1.maven.org/maven2/org/jetbrains/kotlin/kotlin-stdlib/2.2.10/kotlin-stdlib-2.2.10.jar \
    9c67cc79efd6b9215b49d2a4308f5f3433537376c7c88e89bdd6729bd096e61a)
rm -rf "$out/rpv"; mkdir -p "$out/rpv" "$res/xml"
unzip -qo "$aar" classes.jar res/xml/network_security_config.xml -d "$out/rpv"
# its network security config: cleartext only to the CAs' revocation-list hosts, which the verifier's checks fetch
cp "$out/rpv/res/xml/network_security_config.xml" "$res/xml/"
rm -f "$out/res.zip"; "$BT/aapt2" compile --dir "$res" -o "$out/res.zip"
cat >"$out/AndroidManifest.xml" <<'EOF'
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="io.github.fizl0.openmx"
    android:versionCode="1" android:versionName="0.1">
  <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="36"/>
  <uses-feature android:glEsVersion="0x00030000"/>
  <uses-permission android:name="android.permission.INTERNET"/>
  <!-- opentennis's B455: mDNS (LAN lobbies) needs Wi-Fi multicast (MxActivity's MulticastLock) -->
  <uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE"/>
  <!-- opentennis's M140: the in-app update hands the new .apk to the system's installer (MxActivity.install) -->
  <uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES"/>
  <application android:label="openMX" android:icon="@mipmap/ic_launcher"
      android:roundIcon="@mipmap/ic_launcher_round" android:networkSecurityConfig="@xml/network_security_config" android:hasCode="true" android:extractNativeLibs="false"
      android:theme="@android:style/Theme.NoTitleBar.Fullscreen">
    <activity android:name="io.github.fizl0.openmx.MxActivity" android:exported="true" android:screenOrientation="sensorLandscape" android:launchMode="singleTop"
        android:configChanges="orientation|screenSize|screenLayout|keyboardHidden|keyboard|navigation|uiMode"
        android:resizeableActivity="true">
      <meta-data android:name="android.app.lib_name" android:value="openmx"/>
      <intent-filter><action android:name="android.intent.action.MAIN"/><category android:name="android.intent.category.LAUNCHER"/></intent-filter>
    </activity>
  </application>
</manifest>
EOF
rm -rf "$out/classes" "$out/dex" && mkdir -p "$out/classes" "$out/dex"
# opentennis's B618: every lint on and fatal, so a deprecated call shows here and not first in a release log
"${jdk}javac" -Xlint:all -Werror --release 17 -classpath "$JAR" -d "$out/classes" $(find crates/openmx/android -name '*.java')
# R8 rather than d8: the activity's classes and the verifier's whole, of the Kotlin stdlib only what they use
(cd "$out/classes" && "${jdk}jar" cf ../app.jar .)
printf '%s\n' '-dontobfuscate' '-dontwarn org.jetbrains.annotations.**' '-keep class io.github.fizl0.openmx.** { *; }' \
    '-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }' >"$out/r8.pro"
"${jdk}java" -cp "$BT/lib/d8.jar" com.android.tools.r8.R8 --release --min-api 26 --lib "$JAR" --pg-conf "$out/r8.pro" \
    --output "$out/dex" "$out/app.jar" "$out/rpv/classes.jar" "$kotlin"
# a release (tools/release.sh, OPENMX_VERSION=v1.2.3) names its version; versionCode 10203 keeps each release an upgrade
ver=${OPENMX_VERSION:-}; ver=${ver#v}; ver=${ver%%-*}
vargs=()
[[ $ver =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]] && vargs=(--replace-version --version-name "${OPENMX_VERSION#v}" \
    --version-code $((BASH_REMATCH[1] * 10000 + BASH_REMATCH[2] * 100 + BASH_REMATCH[3])))
"$BT/aapt2" link -o "$out/base.apk" --manifest "$out/AndroidManifest.xml" -I "$JAR" "${vargs[@]}" "$out/res.zip"
(cd "$out" && zip -q -0 -r base.apk lib && zip -q -j base.apk dex/classes.dex)
"$BT/zipalign" -f -p 4 "$out/base.apk" "$out/aligned.apk"
key=$HOME/.android/debug.keystore
[ -f "$key" ] || { mkdir -p "${key%/*}"; "${jdk}keytool" -genkeypair -keystore "$key" -storepass android -alias androiddebugkey -keypass android \
    -keyalg RSA -validity 10000 -dname "CN=Android Debug,O=Android,C=US" >/dev/null; }
"$BT/apksigner" sign --ks "$key" --ks-pass pass:android --out dist/openmx.apk "$out/aligned.apk"
echo "dist/openmx.apk ($(du -h dist/openmx.apk | cut -f1); ${OPENMX_ABIS:-arm64-v8a})"
[ -z "$install" ] || "$SDK/platform-tools/adb" install -r dist/openmx.apk
exit 0
