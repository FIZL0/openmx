# 1 — n4 in a browser (PART)

Task N4. 2026-10-20.

**Blocked on N2 and N3.** The brief says "once N2 and N3 run on desktop". When this session started, N2
(rollback) was in s3 with ggrs half-vendored and nothing committed, and N3 (transport and lobby) was in s2
with nothing written. There is no `crates/openmx/src/net` to bring to the browser yet. P36 (the front end) is
also rewriting `crates/openmx/src/main.rs` in s1 (+344 lines, uncommitted). So this session did the parts that
stand alone, in new files, and left `main.rs` alone.

## What I did

- **A wasm32 build of the shell, gated in `./kit check`.** `crates/openmx` gains a library target,
  `openmx_web` (`src/web.rs`, cdylib + rlib), next to the unchanged `openmx` binary. `tools/web.sh` builds it
  for `wasm32-unknown-unknown`. This machine's Arch rustc has no wasm32 std and cannot load an upstream one,
  so the script fetches the matching upstream rustc + cargo + the two stds once into
  `$TMPDIR/openmx-cross/rust-<version>`. That is the directory and layout `tests/cross/platforms.sh` already
  uses, so the two share it. The fetch is serialised with `flock`, because five slots' checks can reach it at
  once. The build goes to `target/web`. The output is a 245 KB module.
- **No wasm-bindgen.** The workspace has no outside dependencies, and wasm-bindgen-cli is not installed here.
  The module imports four plain-number host calls under `openmx`: `disc_size`, `disc_read(at: f64, ptr, len)`,
  `present(ptr, w, h)` and `log`. It exports `openmx_start`, `openmx_frame` and `openmx_message`. The disc
  offset is an `f64` because the image is 2.2 GB, which is past what an i32 can address.
- **The disc from browser storage.** `game_render::disc` gets a `Disc::read_from(Box<dyn Image>)`, where
  `Image` is any `Read + Seek`. `Disc::open` is now that call over a file, so the desktop's behaviour is
  unchanged and the error text is the same. `configured_image()` exposes the kit.toml path for the test. Inside
  the module, `HostImage` is a `Read + Seek` over `disc_read`.
- **`web/`.** These are written in the shape of opentennis's `web/` but are new files:
  - `index.html`: a disc picker, the canvas, and `body[data-state]` for the tests.
  - `worker.js`: copies the picked ISO into OPFS `disc.iso`, 8 MB at a time through a sync access handle,
    under `.part` until the copy is complete. It then runs the module and reads the disc through the handle.
    If OPFS is refused (a private window), it reads the picked `File` through `FileReaderSync` and copies
    nothing.
  - `sw.js` (network-first offline cache), `manifest.webmanifest` and `icon.svg`.
  - `web/.gitignore` for `pkg/`. This is its own file so that the root `.gitignore`, which P36 is editing, is
    left alone.
- **What it draws:** `openmx_web::start_line` draws free06 from behind the start line: the desktop shell's
  own `ground.rs` (compiled into the library as well) and then `draw_scene` over the scene's props. **This is
  not the ride.**

### What came from opentennis and what changed

- **Taken:**
  - the `web/` layout: page, worker, service worker, manifest;
  - worker.js's `.part`-then-`move(dir, name)` with the WebKit fallback, copied almost verbatim;
  - `tools/web.sh` as the entry point.
- **Changed:**
  - **No wasm-bindgen and no wasm-opt.** The ABI is hand-written imports and exports, the way this
    workspace does everything else.
  - **The disc is stored as-is in OPFS.** opentennis writes its own pack format; here the disc reader already
    reads the ISO by sector, so a raw copy is enough.
  - **The private-window fallback reads the picked File with FileReaderSync.** opentennis builds an in-memory
    Blob instead; this way nothing is copied.
  - **The service worker is network-first and about 20 lines.** opentennis has a 137-line staged-build
    updater.
- **Not taken yet:**
  - **`touch.js`'s virtual pad.** It feeds gilrs through `navigator.getGamepads()`. Our shell has its own pad
    layer, which the browser cannot reach until the ride runs there.
  - **`net/rtc.rs` and relay-only iroh.** There is no net layer on desktop to port yet.

## What the original does

Nothing in this task touches the simulation. The browser build compiles the same `game-sim` and
`game-render` source as the desktop, and `crates/game-sim/tests/platforms.rs` already pins the state hash on
wasm32 (wasip1).

## How it was proved

- **The build gate.** `crates/openmx/tests/web.rs::the_shell_builds_for_the_browser` runs on every
  `./kit check`. It runs `tools/web.sh --check` and decodes the module's import section (id 2) and export
  section (id 7). Every import must be one of the four host calls `worker.js` supplies, and every export the
  page calls must be present. An `env.*`, `__wbindgen_*` or WASI import, which would make the module
  impossible to instantiate in the page, fails it.
- **The browser draws what the desktop draws.** `the_browser_draws_what_the_desktop_draws` runs the wasm
  module in node, with `tests/web/host.mjs` answering the same imports from a file instead of the OPFS handle.
  It runs over the user's disc and compares the result with `start_line` drawn natively. The result was
  136,557 of 286,720 pixels drawn and **0 differing, max difference 0**. The test tolerates up to 1% of
  pixels differing by up to 64, because the two builds use different libm `sin`/`cos`. It prints what it
  measured.
- **The page in a real browser.** `the_page_starts_in_a_browser` is ignored and run by hand; it passed today.
  It drives headless chromium over CDP (`tests/web/chromium.mjs`, which uses node's own WebSocket and nothing
  else):
  - On the first visit the page asks for the disc. The ISO is picked through `DOM.setFileInputFiles` and
    copied into OPFS in 1.4–3.5 s on tmpfs, with `navigator.storage.estimate()` reporting 2,220,194,241 bytes.
    Then it draws.
  - A reload starts in 0.22–0.33 s without asking and gives the identical screenshot (same sha-256).
  - With OPFS refused (`--no-opfs`) the page plays off the picked file, gives the same screenshot, and asks
    again on reload.
  - I looked at the screenshot myself: it is free06's dunes under its grey fog.
- `./kit check`: TESTS PASSED.

## Not verified / not 1:1

- **The browser does not ride.** The desktop loop is one 1,150-line `run()` that owns its window (X11
  socket), clock (`Instant`), pad (`/dev/input`) and tapes. Stepping it a frame at a time from a host it does
  not own means restructuring `main.rs`. P36 and N2 are both rewriting that file right now, so doing it here
  would have meant a three-way conflict. The browser therefore draws one still of the start line, through the
  same ground and scene passes the desktop uses. It is not the ride, and there is no pad in the browser.
- **No online play in the browser.** There is no netplay at all yet: N2 and N3 are unmerged. That leaves
  three things undone:
  - the browser transport (iroh relay-only, browser-to-browser LAN over WebRTC as opentennis's `net/rtc.rs`
    does it);
  - the brief's test, a desktop and a browser peer agreeing on hashes in `crates/openmx/tests/netplay.rs`;
  - the room-code lobby in the page.
- **`touch.js` (virtual pad) not brought over.** There is nothing in the browser for it to drive yet.
- **The chromium test is not in `./kit check`.** It copies 2.2 GB into a scratch profile on every run, so it
  is `#[ignore]`d. The node-hosted test is in the check, but node is not a browser: it does not exercise OPFS,
  the worker, OffscreenCanvas or the service worker. Only the ignored test does.
- **Only Chromium was tried.**
  - Firefox and Safari were not tried, and neither was a phone.
  - On WebKit, `FileSystemSyncAccessHandle` returned promises in its first versions. `worker.js` calls
    `truncate`, `write`, `flush`, `getSize` and `read` synchronously, so on those versions it would break.
    opentennis's `opfs.js` awaits those calls for that reason.
- **The offline path (`sw.js`) was not exercised.** The service worker registered in the chromium runs (CDP
  attached to it), but no run went offline to check that the cache serves the page.
- **The desktop and the browser drew bit-identical pixels on this machine.** Another machine's libm could
  move an edge, and the test's tolerance exists for that. That tolerance has not been seen in use.
- **The fetched toolchain needs network once per machine,** or after `/tmp` is cleared. Offline with nothing
  cached, `the_shell_builds_for_the_browser` fails and says so. It is deliberately not a skip, because a build
  gate that skips is not a gate. `OPENMX_CROSS_DIR` can point at a persistent directory.

## Next

1. Once P36 and N2 are merged: split `main.rs`'s `run()` into a `Shell` built from a `Disc` plus options, with
   a `frame(report, now) -> &Frame` step. The X11 window, `/dev/input` pads, `Instant` and the tape files
   stay in `main.rs`'s host loop. `web.rs` then drives the same `Shell` from `requestAnimationFrame` in the
   worker, with `performance.now()` as the clock and a pad report posted from the page (Gamepad API and
   keyboard; `touch.js` later). `start_line` goes away.
2. Once N3 is merged: add the browser transport behind the same trait N3's iroh transport implements. That
   means relay-only to desktop peers, and WebRTC data channels browser to browser, following opentennis's
   `crates/hst-base/src/net/rtc.rs`. Then the brief's test: a desktop peer and the wasm build, run in node or
   chromium, agreeing on every hash in `crates/openmx/tests/netplay.rs`.
3. `tests/web/chromium.mjs` is the harness for that browser peer: the same CDP driving, with the page told to
   join a room.
