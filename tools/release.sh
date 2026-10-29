#!/bin/bash
# tools/release.sh VERSION [notes.md] [publish] — a full release (v1.0.0, "openMX 1.0.0"), as opentennis's tools/release.sh:
# set the workspace's version to VERSION (a "Release vVERSION" commit if it changes), build every platform tools/build.sh
# builds today (Linux; Windows once its port lands) and the website (tools/web.sh), sign SHA256SUMS with the minisign key
# (~/.config/openmx/release.key, asks its password unless ~/.config/openmx/release.pass holds it) and upload to GitHub:
#   openmx-linux-x86_64.tar.gz   what players download
#   openmx-linux-x86_64          the bare binary, for the in-game updater (plan/TODO.md) to fetch
#   openmx-ios.ipa               P201: the unsigned iPhone / iPad app (tools/ios-ipa.sh; needs the iOS SDK, tools/ios-sdk.py)
#   altstore-source.json         P201: that app as an AltStore / SideStore / Feather source (tools/ios-source.sh). FIZL0's
#                                shared source (github.com/FIZL0/apps) merges it; players add https://fizl0.github.io/apps/source.json
#                                (web/ios.html), which a published release rebuilds at once (`gh workflow run source -R FIZL0/apps`)
#   SHA256SUMS  SHA256SUMS.sig
#   web.tar.gz                   the website, which pages.yml deploys to openmx.dev as built here
# VERSION with a suffix (1.1.0-rc.1) is a pre-release, never Latest. notes.md is the "New in …" list (Markdown
# bullets); without it, RELEASE_NOTES.md (kept up to date as changes land; emptied and committed after the release),
# and if that has no bullets either, the commit subjects since the last v* tag (required for the first release). A draft
# unless `publish` is given: check it on GitHub and publish there. Publishing it puts its website on openmx.dev
# (pages.yml's release run). Every push to main is a beta (pages.yml: the `beta` release and beta.openmx.dev), not made here.
set -e
cd "$(dirname "$0")/.."
key=~/.config/openmx/release.key
pubfile=~/.config/openmx/release.pub
repo=FIZL0/openmx
notes= publish=--draft version=
for a; do
    case $a in
        publish) publish=--latest ;;
        v[0-9]*|[0-9]*) version=${a#v} ;;
        *) notes=$a ;;
    esac
done
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "usage: tools/release.sh VERSION [notes.md] [publish]  (VERSION like 1.0.0)" >&2; exit 1; }
tag=v$version
pre=
[[ $version == *-* ]] && { pre=--prerelease; [ "$publish" = --draft ] || publish=--latest=false; }
[ -n "$notes" ] || ! grep -q '^- ' RELEASE_NOTES.md || notes=RELEASE_NOTES.md
last=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)
[ -n "$notes" ] || [ -n "$last" ] || { echo "the first release needs a notes.md or bullets in RELEASE_NOTES.md (no earlier v* tag to list commits from)" >&2; exit 1; }
[ -z "$notes" ] || [ -f "$notes" ] || { echo "no such notes file: $notes" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq missing: sudo pacman -S jq" >&2; exit 1; }
sdk=${OPENMX_IOS_SDK:-$HOME/.local/share/hst-ios/iPhoneOS.sdk}
[ -d "$sdk" ] || { echo "no iOS SDK at $sdk (the release carries openmx-ios.ipa): run tools/ios-sdk.py Xcode.xip" >&2; exit 1; }
command -v minisign >/dev/null || { echo "minisign missing: sudo pacman -S minisign" >&2; exit 1; }
[ -f "$key" ] && [ -f "$pubfile" ] || { echo "no signing key at $key (once: mkdir -p ~/.config/openmx && minisign -G -p $pubfile -s $key)" >&2; exit 1; }
pub=$(tail -1 "$pubfile")
# the key's password up front, checked on a scratch file, so a typo fails now and not after the builds
pwfile=~/.config/openmx/release.pass
if [ -f "$pwfile" ]; then pw=$(<"$pwfile"); else read -rsp "release key password: " pw; echo; fi
t=$(mktemp); trap 'rm -f "$t" "$t.minisig"' EXIT
printf %s "$pw" | minisign -S -s "$key" -m "$t" >/dev/null 2>&1 || { echo "wrong password for $key" >&2; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "commit first: the release is tagged at HEAD" >&2; exit 1; }
# `gh release view` can't see drafts (they have no tag yet); the list can
! gh release list -R $repo -L 100 --json tagName -q '.[].tagName' | grep -qx "$tag" || { echo "$tag already has a release (a draft?): delete or publish it on GitHub first" >&2; exit 1; }

# the shell reports the workspace's version (the online handshake's build id), so it must be the tag's
if ! grep -qx "version = \"$version\"" Cargo.toml; then
    sed -i "0,/^version = \".*\"/s//version = \"$version\"/" Cargo.toml
    cargo metadata -q --offline --format-version 1 >/dev/null  # rewrites the workspace crates' entries in Cargo.lock
    git commit -q -m "Release $tag" Cargo.toml Cargo.lock
fi

# one commit stamp for every platform, the browser's too (the online build id: players of one release meet)
export OPENMX_COMMIT=$(git rev-parse HEAD | cut -c1-7) OPENMX_VERSION=$tag
tools/build.sh all
out=dist/release
rm -rf "$out" && mkdir -p "$out"
tar -czf "$out/openmx-linux-x86_64.tar.gz" -C dist/linux .
cp dist/linux/openmx "$out/openmx-linux-x86_64"
tools/ios-ipa.sh "$out/openmx-ios.ipa"
# P201: the sideloading source, at a URL that always serves the latest release's copy (FIZL0/apps reads it there)
tools/ios-source.sh "$version" "$out/openmx-ios.ipa" >/dev/null
tools/web.sh
tar -czf "$out/web.tar.gz" -C web .
(cd "$out" && sha256sum openmx-* web.tar.gz > SHA256SUMS)
printf %s "$pw" | minisign -S -s "$key" -m "$out/SHA256SUMS" -x "$out/SHA256SUMS.sig" -t "openMX $tag"
minisign -V -P "$pub" -m "$out/SHA256SUMS" -x "$out/SHA256SUMS.sig" -q || { echo "SHA256SUMS.sig doesn't verify against $pubfile" >&2; exit 1; }
# the game checks against the key built into it once the updater lands (plan/TODO.md): refuse a release it would turn down
upd=crates/openmx/src/update.rs
if [ -f $upd ]; then
    minisign -V -P "$(sed -n 's/^pub const PUBLIC_KEY: &str = "\(.*\)";/\1/p' $upd)" -m "$out/SHA256SUMS" -x "$out/SHA256SUMS.sig" -q \
        || { echo "SHA256SUMS.sig doesn't match $upd's PUBLIC_KEY: wrong key?" >&2; exit 1; }
fi

body=$out/notes.md
{
    [ -z "$pre" ] || { echo "> **Pre-release:** a test build of the next version."; echo; }
    echo "openMX is *MX Unleashed* (PS2, 2004) rewritten from scratch: the original's physics, tricks and timing, proved"
    echo "tick for tick, on today's screens at any resolution, with split screen, online play and mods."
    echo
    echo "## Download"
    echo
    echo "| Platform | File | How to play |"
    echo "|---|---|---|"
    echo "| **Linux** / **Steam Deck** | [\`openmx-linux-x86_64.tar.gz\`](https://github.com/$repo/releases/download/$tag/openmx-linux-x86_64.tar.gz) | Unpack, put your ISO's path in \`kit.toml\`, run \`./run.sh\` |"
    echo "| **iPhone** / **iPad** (iOS 15+) | [add the source in one tap](https://openmx.dev/ios.html) | Easiest: add the source in one tap (Feather, AltStore, SideStore), or add \`https://fizl0.github.io/apps/source.json\` under Sources by hand; install openMX from it and updates arrive there. Then put your ISO in Files → On My iPhone → openMX. Touch controls or a controller. Signing it yourself instead: [\`openmx-ios.ipa\`](https://github.com/$repo/releases/download/$tag/openmx-ios.ipa) (unsigned; Sideloadly, AltStore…) |"
    echo "| **Browser** | [openmx.dev](https://openmx.dev/) | Nothing to install |"
    echo
    echo "## What you need"
    echo
    echo "Your own disc image of *MX Unleashed* (USA, SLUS-20767). **No game data is included**, and the game reads"
    echo "your ISO on your own machine: nothing is uploaded."
    echo
    echo "## New in $version"
    echo
    if [ -n "$notes" ]; then sed '/^<!--/d' "$notes"; else git log --format='- %s' "$last..HEAD" --no-merges; fi
    echo
    echo "## Verify the download"
    echo
    echo "\`SHA256SUMS\` is signed with the project's [minisign](https://jedisct1.github.io/minisign/) key:"
    echo
    echo '```'
    echo "minisign -Vm SHA256SUMS -P $pub && sha256sum -c --ignore-missing SHA256SUMS"
    echo '```'
    echo
    echo "Want the newest changes before the next release? Try the [beta](https://github.com/$repo/releases/tag/beta) or [beta.openmx.dev](https://beta.openmx.dev/)."
} > "$body"
git push -q origin HEAD:main
gh release create "$tag" -R $repo --target "$(git rev-parse HEAD)" --title "openMX $version" \
    --notes-file "$body" $pre $publish "$out"/openmx-* "$out"/web.tar.gz "$out"/SHA256SUMS "$out"/SHA256SUMS.sig "$out"/altstore-source.json
# the shared FIZL0 source (github.com/FIZL0/apps) picks up this release now, not at its next hourly poll
[ "$publish" = --latest ] && { gh workflow run source -R FIZL0/apps || echo "FIZL0/apps did not rebuild; it will within the hour" >&2; }
if [ "$notes" = RELEASE_NOTES.md ]; then
    sed -i '/^<!--/!d' RELEASE_NOTES.md
    git commit -qm "Release notes: emptied after v$version" -- RELEASE_NOTES.md && git push -q origin HEAD:main
fi
echo "$tag: $([ "$publish" = --draft ] && echo "draft, publish it on GitHub" || echo published)"
