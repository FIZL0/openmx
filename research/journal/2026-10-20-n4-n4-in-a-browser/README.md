# N4 — n4 in a browser

2026-10-20. Task: `./kit plan N4`.

openmx in a browser, playing online against desktops. Part 1 built the shell for wasm32 (`tools/web.sh`,
gated in `./kit check`), read the user's disc from OPFS and drew the start line in a canvas, the same pixels
as the desktop. Part 2 came after N2 and N3 merged. It ported N3's iroh transport to wasm32 (relay-only, no
DHT or mDNS), switched the build to wasm-bindgen (ggrs and iroh need it), and added `peer.rs`: N2's session
on N3's transport, the same code on both builds. It also added the brief's test: a desktop and the page
itself in headless chromium (plus the module alone in node) ride 600 ticks through a local relay and agree on
every tick's hash and pads. Not done: the browser rides N2's flat test ride on a bot pad, not the disc's
track with a real pad, because that needs `main.rs`'s `run()` split and three other slots are editing it.
There is no WebRTC between browsers and no `touch.js`. Details, and everything not proved, in the numbered
files; the newest one's suffix says whose move it is (FINAL · PART · READY).
