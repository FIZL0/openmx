#!/usr/bin/env bash
# tools/beta-bundle.sh WEB_TAR OUT SHA BUILD... — the whole of https://beta.openmx.dev as one tarball, for pages.yml's
# beta job, as opentennis's (B43, its B745: the beta lives there, not in a GitHub release, which launchers such as
# Quiver took for the newest version): the website from WEB_TAR, and under download/ the beta's builds, a page listing
# them (download/index.html), files.txt (their names) and commit.txt (the full commit they were built from; Actions
# builds' update notice reads it, crates/openmx/src/update.rs).
set -euo pipefail
[ $# -ge 4 ] || { echo "usage: tools/beta-bundle.sh WEB_TAR OUT SHA BUILD..." >&2; exit 1; }
web=$1 out=$(realpath -m "$2") sha=$3
shift 3
[[ $sha =~ ^[0-9a-f]{40}$ ]] || { echo "not a full commit: $sha" >&2; exit 1; }
d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT
tar xzf "$web" -C "$d"
dl=$d/download
mkdir -p "$dl"
: > "$dl/files.txt"
for f in "$@"; do
    n=$(basename "$f")
    [[ $n =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo "bad file name: $n" >&2; exit 1; }
    cp "$f" "$dl/$n"
    echo "$n" >> "$dl/files.txt"
done
echo "$sha" > "$dl/commit.txt"
what() {
    case $1 in
        openmx-windows-x86_64.zip) echo "Windows 10/11|Unzip, then drop your ISO on <code>openmx.exe</code> once; start <code>openmx.exe</code> after" ;;
        openmx-linux-x86_64.tar.gz) echo "Linux / Steam Deck|Unpack, run <code>./openmx your.iso</code> (or drop the ISO on it) once; <code>./openmx</code> after. For Steam (with artwork): run <code>add-to-steam.sh</code>" ;;
        *) echo "|" ;;
    esac
}
rows=
while read -r n; do
    IFS='|' read -r plat how <<< "$(what "$n")"
    size=$(numfmt --to=iec --suffix=B "$(stat -c %s "$dl/$n")")
    rows+="<tr><td>${plat:-&nbsp;}</td><td><a href=\"$n\">$n</a> <span class=dim>$size</span></td><td>$how</td></tr>"$'\n'
done < "$dl/files.txt"
cat > "$dl/index.html" <<EOF
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>openMX beta downloads</title>
<style>
:root { --bg: #fafafa; --fg: #1d1d1f; --dim: #6e6e73; --line: #d2d2d7; --accent: #c2410c; color-scheme: light dark }
@media (prefers-color-scheme: dark) { :root { --bg: #161617; --fg: #f0f0f2; --dim: #a1a1a6; --line: #3a3a3c; --accent: #fb923c } }
body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.5 system-ui, sans-serif }
main { max-width: 60rem; margin: 0 auto; padding: 2rem max(16px, env(safe-area-inset-right, 0px)) 3rem max(16px, env(safe-area-inset-left, 0px)) }
a { color: var(--accent) }
.dim { color: var(--dim) }
.wrap { overflow-x: auto }
table { border-collapse: collapse; width: 100% }
td { border-top: 1px solid var(--line); padding: .6rem .5rem; vertical-align: top }
td:first-child { font-weight: 600; white-space: nowrap }
code { font-size: .9em }
</style>
</head>
<body>
<main>
<h1>openMX beta</h1>
<p><strong>Rebuilt on every change to main (<a href="https://github.com/FIZL0/openmx/commit/$sha">${sha::7}</a>). Newer than the latest release, less tested.</strong>
For the stable version, see <a href="https://github.com/FIZL0/openmx/releases/latest">the latest release</a> (it also has the Mac, iPhone and Android apps).</p>
<p>No game data is included: you need your own <em>MX Unleashed (USA)</em> (SLUS-20767) ISO.
Or play the beta <a href="../">in the browser</a>.</p>
<p class="dim">Every build here runs the native sim (the host's plain floats: faster, not bit-exact with the PS2). It plays online only with other native builds, and a tape recorded on an exact build warns when played.</p>
<div class="wrap"><table>
$rows</table></div>
</main>
</body>
</html>
EOF
tar czf "$out" -C "$d" .
echo "$out"
