# 2 — online in a browser (FINAL)

Task N4. 2026-10-20.

N2 (rollback: `crates/openmx/src/net`, ggrs vendored) and N3's transport (`crates/openmx-net`: iroh, the
lobby, `Cmd::Datagram`/`Event::Datagram` relayed through the host) have merged since part 1. This part ports
the transport to wasm32 and adds a peer that runs the same code on both builds. It also adds the brief's test:
a desktop and a browser ride together and agree on every tick.

## What I did

- **wasm-bindgen after all.** Part 1 used a hand-written ABI with no outside dependencies. That cannot hold
  now: ggrs needs its `wasm-bindgen` feature on wasm32 (for `js-sys`, `instant` and `getrandom/js`), and
  iroh's wasm build uses `web-sys` for the relay's WebSocket. Both pull in `wasm-bindgen` imports that only
  the bindgen glue can satisfy. opentennis uses the same setup.
  - `tools/web.sh` now runs `wasm-bindgen --target web` after the cargo build. It uses the CLI from PATH when
    its version matches `Cargo.lock`. Otherwise it runs `cargo install --locked wasm-bindgen-cli --version
    <lock's>` into the shared `$TMPDIR/openmx-cross/` scratch, under the same `flock` the toolchain fetch
    uses, and honours `OPENMX_CROSS_OFFLINE`.
  - The output is `pkg/openmx_web.js` plus `pkg/openmx_web_bg.wasm`, 6.46 MB. Part 1's module was 245 KB;
    iroh, quinn and rustls make up the difference. No wasm-opt.
  - The host calls (`disc_size`, `disc_read`, `present`, `log`) are now bindgen imports from the global
    `openmx`, and `disc_read` takes the `&mut [u8]` directly.
  - The gate in `tests/web.rs` is the same in spirit as before. Every import of the module must come from
    the glue (`./openmx_web_bg.js`), and the calls the glue makes on `openmx.` must be exactly the four host
    calls. `worker.js` and `host.mjs` answer those four.
- **openmx-net builds for wasm32-unknown-unknown.** Every change is behind `cfg(target_arch = "wasm32")`; the
  native code paths are unchanged.
  - Timers, sleeps, `Instant`, `SystemTime` and `spawn` go through `n0_future` (tokio natively, the page's
    event loop on wasm). std's `Instant`, `SystemTime` and `process::id` panic on wasm32-unknown-unknown.
  - On wasm the mainline DHT (the board) and mDNS (LAN) are not built. `Dht` and `MdnsAddressLookup` become
    empty enums, so the code that would use them is `match x {}`, and a wasm endpoint is relay-only with no
    address lookups.
  - `tokio` keeps only `macros` and `sync` in common; the runtime and `time` are native-only. `Net::next`
    (a blocking wait) is native-only; the page polls.
  - `crates/openmx/src/net/desync.rs` takes the time from `js_sys::Date` and uses pid 0 on wasm.
- **`crates/openmx/src/peer.rs`: one machine riding online.** A `Peer` holds a `Net` and, once the host's
  Start comes, a `Netplay` on N2's `Rides`. One frame does three things: the transport's events go to the
  session, the session runs its ticks on this machine's pads, and its datagrams go back out as
  `Cmd::Datagram`. The desktop test and the page run this same code.
  - `peer::ticks(np, from, to)` gives one line per tick: the state's hash before the tick, and FNV-1a of
    every seat's pad bytes.
  - `peer::mash` is N2's test pad.
  - Anything longer than `openmx_net::net::DATAGRAM_MAX` (1100) is dropped and counted in `too_big`.
- **`web.rs`: `Online` for the page.** `new(name, relay, code_servers)` sets up relay-only with no board and
  no LAN, then the page calls `host`, `join`, `ready`, `start`, `leave`, `frame(dt)`, `state()` (a JS object)
  and `ticks(from, to)`.
- **The page.**
  - `index.html?join=<code or ticket>` (or `?host=<seats>`, with `&relay=`, `&codes=`, `&name=`, `&bike=` and
    `&ticks=`) rides online instead of starting the disc's still. The lobby's state shows in the corner, in
    `body[data-online]` and in `window.openmxOnline`.
  - `worker.js` is now a module worker. It imports the glue and runs `Online.frame` every 60th of a second
    by `performance.now()`. A guest readies as soon as it is seated; a host starts once every seat is taken
    and ready.
  - `sw.js` caches the two pkg files.
- **The test: `crates/openmx/tests/netplay_browser.rs`.**
  - The desktop is the test process: a `Peer` on native `Net`, hosting two seats through iroh's relay
    server, run in-process on loopback over plain HTTP as openmx-net's own tests do.
  - The browser joins by the host's ticket, and every tick of the first 600 (10 s) must be the same line on
    both. Neither side may see a desync or drop an oversized datagram, and the hashes must take more than 300
    distinct values (the bikes move, so the comparison is about something). It runs twice:
    - `a_desktop_and_a_browser_ride_the_same_ride`: **the page itself in headless chromium**
      (`tests/web/page.mjs`). web/ is served with `/pkg/` from the `--check` build, the page is opened at
      `?join=…&ticks=600`, and `window.openmxTicks` is read over CDP. This path covers index.html,
      worker.js, the module and the glue.
    - `the_browser_build_rides_the_same_ride_in_node`: the same module and the worker's frame loop in node
      (`tests/web/peer.mjs`). A failure only in the chromium test is therefore the page's fault, not the
      module's.
  - It is a new file, not part of `tests/netplay.rs` as the brief says: N5 is rewriting `netplay.rs` in s5
    right now. It includes `src/net/mod.rs` and `src/peer.rs` by path, the way `netplay.rs` includes `net`.

## How it was proved

- `./kit check`: **TESTS PASSED** (whole workspace) with both new tests in it.
- Both tests took 10.5 s (node) and 11.5 s (chromium) of riding. Lobby to Go took about 0.2 s; the browser
  was seated 0.6 s after the desktop was. In a 2-minute run, logged every 2 s, the ride stayed in step for
  7,000 ticks, the browser's tick within about 80 of the desktop's, with no desync either side.
- To prove a passing test had really run, I made it panic with its own summary and read that in the log. I
  did this for `netplay_browser` and for `the_browser_draws_what_the_desktop_draws`. Through the new glue
  the draw test still gives **136,568 of 286,720 pixels drawn, 0 differing**.
- The ignored disc test (`the_page_starts_in_a_browser`) ran by hand through `./kit check -- … --ignored`:
  - The OPFS visit and the reload both reached `started` with the same screenshot.
  - Its `--no-opfs` half **failed twice** (first two runs): status `TypeError: Failed to fetch` 0.25 s after
    the file was picked. After that, three runs of the test and eight by hand (including the
    OPFS-then-no-OPFS order with one profile path) all passed. See below.
- **A test bug I made and fixed:** the browser reported its ticks the moment `confirmed >= 600`, but
  confirmed can be one ahead of the ticks run there (tick 599, confirmed 600). In that case it sent 599 lines
  once and the test waited forever. Both sides now wait until every line exists. Six runs since have
  passed.

## Not verified / not 1:1

- **The browser rides N2's test ride, not the game.** That ride is one `Sim` per seat on a flat floor with
  no disc. The disc track and the drawn ride need `main.rs`'s `run()` split into a steppable `Shell`. Three
  other slots are editing `main.rs` right now (P44 GPU renderer, S53 ragdoll, N5 codec), so I did not touch
  it. The page's online mode draws nothing of the ride.
- **The page's seat rides a bot pad** (`peer::mash`). There is no Gamepad API, keyboard or `touch.js`
  virtual pad in the page yet. That has to wait for the `Shell` too, since the pad layer lives in `run()`.
- **No WebRTC between browsers** (opentennis's `net/rtc.rs`). Browser↔desktop and browser↔browser both go
  through the relay. Two browsers on one LAN pay the relay's round trip. Nothing tested browser↔browser.
- **Only a local relay over plain HTTP.** The page against n0's public relays (https/wss), and room codes
  through a pkarr relay from the page, were not tried: tests run offline. The test joins by ticket, so the
  page's room-code join (`reqwest` → `fetch`) never ran. A page served over https would need a wss relay;
  a plain-http relay is mixed content there.
- **The handshake's disc check is names-only on the page** (`openmx_net::Names`): the page does not hash its
  OPFS disc's bike files. The desktop in the test uses `Names` as well, so a mismatched disc between a page
  and a desktop is not caught yet.
- **Datagram sizes disagree.** N2's link pieces ggrs messages at 1200 bytes (`net/link.rs` `DATAGRAM_MAX`),
  but openmx-net carries at most 1100. `Peer` drops and counts anything bigger. With two seats nothing was
  dropped (`too_big` 0, asserted). With eight seats, or a long stall, a ggrs input message between 1100 and
  1200 bytes would be lost every time. One of the two constants should change; neither file is this task's.
- **A desync report on the page goes nowhere**: no file system. `dumps` is empty, so the report is not
  written. It should go to the console or a download.
- **The `--no-opfs` failure is unexplained.** In two consecutive runs right after the module changed, the
  private-window path got `TypeError: Failed to fetch`. After that I could not make it happen again, even
  with CDP attached to the worker to watch its network. My guess is a race with the service worker
  (`skipWaiting` + `clients.claim()` while the worker fetches the 6.5 MB module), but that is unproved. The
  test is ignored and run by hand.
- **Only Chromium (and node).** Firefox, Safari and phones were not tried. Module workers need Firefox 114+
  and Safari 15+. Part 1's WebKit `FileSystemSyncAccessHandle` caveat still applies.
- **The module grew from 245 KB to 6.46 MB** without wasm-opt. Nobody has measured load time over a real
  network.
- **wasm-bindgen-cli must match `Cargo.lock`.** If it is not on PATH, the first build installs it (network,
  about a minute). An offline machine without it fails the build gate, deliberately and with a message.
- **N3's status line stays "Starting: checking the ride…" after Go** on both sides. That is N3's text, not
  changed here. The page shows it.

## Next

1. Split `main.rs`'s `run()` into a `Shell` (part 1's plan) once P44, S53 and N5 have merged. Then the page
   rides the disc track with a real pad (Gamepad API, keyboard, `touch.js`), and `Peer` steps the `Shell`
   instead of `Rides`.
2. Make the two `DATAGRAM_MAX`s agree. N2's link should piece at the transport's size.
3. WebRTC data channels between browsers, signalled through the host's lobby (opentennis `net/rtc.rs`).
4. The page's disc in the handshake: hash the OPFS disc's bike files the way the desktop's `Disc` does.
