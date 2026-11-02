# 1 — online menu in two pages (FINAL)

Task P204. 2026-11-02.

## What I did

- `crates/openmx/tests/web/online.mjs` (new) is the driver. It serves `web/` on loopback, with the index's
  `opfs = true;` switched off as gpu.mjs/store.mjs do, so the disc is read in place and not copied twice.
  It opens two pages in one chromium. Each page has its own `Target.createBrowserContext`, which gives it
  its own OPFS, as two people's browsers would have. Each page is attached flat, with its worker
  auto-attached so the shell's console can be read. The URL is `?args=--screen online`.
  - Before the disc is picked, the driver writes OPFS `files/online.txt` on each page:
    `relay = own`, `relay_url = <the test's relay>`, `code_server = <origin>/pkarr`. The worker loads its
    files only once the disc is picked.
  - `/pkarr/<key>` on the same server stands in for a pkarr relay. It keeps PUTs in memory and does not
    check signatures. Because it is same-origin, the page needs no CORS.
  - Chromium runs with `--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1`. The shell's code
    servers are `code_server` and then relay.pkarr.org, and that public one is never reached. A failed
    put there leaves `publish` Ok as long as the local one took it. Nothing leaves the machine.
  - The keys are CDP `Input.dispatchKeyEvent`: arrows, Space (Cross), Return (Start) and W (gas). Each
    tap is held 120 ms with a 300 ms rest, because the menu debounces re-presses
    (menu-taps-need-a-rest-since-p109). `Emulation.setFocusEmulationEnabled` is set so keys reach both
    windows.
  - The flow:
    1. Host: Down, Space (Online → Host), then wait for `lobby: room`.
    2. Wait until the code server holds a PUT. A lookup before that finds nothing.
    3. Guest: Down×3, Space (Join by Code). The host's last `lobby: code` is typed on the on-screen
       keyboard by grid moves from (0,0), then Return.
    4. Guest: wait for 2 seated, then Down×2, Space (Ready).
    5. Host: wait for `ready true`, then Down×6, Space (Start).
    6. Both: W held, then wait for `state before tick 600` on both pages.
  - It prints `{ code, host: [...], guest: [...] }`.
  - It fails with a reason after two minutes with no new console line, or ten minutes in all. It quits
    chromium and removes its profile on every way out.
- `crates/openmx/tests/netplay_browser.rs` `two_pages_host_and_join_from_the_online_menu` runs the
  driver, using `local_relay()`, the module from `tools/web.sh --check`, the GPU flags of web.rs's pad
  test, and the `Node` drop guard. It checks three things:
  - the host's and the guest's hashes before ticks 300 and 600 are the same;
  - both say `0 desyncs`;
  - the hash at 300 is not the hash at 600.
  It skips without the disc, node or chromium.
- Product side (browser build only; the desktop's output is unchanged):
  - `lobby.rs` `page_line`: a console line for each lobby event the front end hears (`lobby: status`,
    `code`, `me`, `room <code> <n> seated, ready <b>`, `start`, `failed`, `dropped`). `Network::pump`
    says it on wasm.
  - `online.rs` `Online::settled_line`: the hash before the last multiple of 300 ticks at or under
    `Netplay::settled`, which is final where `confirmed` can still be a prediction (K7). `main.rs`'s
    online ride says it on wasm each time it moves to a new multiple.

Taken from opentennis: nothing. It has no browser front-end netplay test. The CDP plumbing is
page.mjs's and store.mjs's.

## What the original does

Nothing to port: this is the port's own browser front end and netcode.

## How it was proved

- `cargo test -p openmx --test netplay_browser two_pages -- --nocapture` passed on its first run, in 80 s.
  The room code was 2CA4. The guest joined at 12.9 s and the start came at 16.9 s. Both pages said
  `0x1a720fb33d7d9073` before tick 300 and `0x3906796e8c0eb0a3` before tick 600, with 0 desyncs.
- `./kit check -- -p openmx --test netplay_browser`: 21 tests passed.
- `cargo build -p openmx` (desktop) builds, and `tools/web.sh --check` builds the wasm module.

## Not verified / not 1:1

- I ran it once by hand and once inside `./kit check`, not under a loaded gate. Its waits are on console
  lines and on the put reaching the code server, not on time, apart from a 2 s pause after the menu's
  first frame before the first key. That pause is the one place it could race on a very slow machine:
  a key before the menu reads presses is lost, and the test then times out waiting for `lobby: room`.
- The page's touch pad, gamepads, LAN, Find Games and Quickplay are not driven. Find Games needs mDNS
  or the DHT, and a browser has neither.
- Both pages ride the same keyboard gas with no steering. The hashes are equal and they change between
  300 and 600, but the ride itself is not checked.

## Next

Nothing.
