# 1 — browser ride reads settled ticks (FINAL)

Task K7. 2026-10-29.

## What I did

No log of S149's failing gate survives (the merge's check.log was overwritten by the next run; the runner's
fixer.log only says "passed on the rerun", and for P213's earlier failure of the same test calls it the "not done
after two minutes" timeout). So I went looking for what the test races, reproducing load on the test alone:
`systemd-run --user --scope -p CPUQuota=N%` around either the whole test binary (both peers starved alike) or,
through a scratch-only env var, around the browser's `node page.mjs` only (one peer starved, the other not, which is
what a busy gate does). About 120 runs in all, 15–65 % quota, up to six at a time.

**Race 1, found and removed: "confirmed" is not "settled".** Both sides read their tick lines (`peer::ticks`: the
hash before each tick and the pads it ran on) as soon as `Netplay::confirmed() >= TICKS`. That is ggrs'
`confirmed_frame()`: the last frame whose pads have *arrived*. `Netplay::frame` polls ggrs (which takes the
datagrams in and moves `confirmed_frame` on) and then the tick gate may run no `advance_frame` at all — a frame
sat out for time sync (`want == 0`) or a B198 hold. The rollback that rewrites `hashes`/`log` with the pads that
just came in only happens inside the next `advance_frame` (third_party/ggrs `advance_rollback_frame` →
`handle_rollback_and_save`). In between, ticks up to `confirmed` still hold their prediction. Instrumented
(scratch only, not committed): with the browser starved, the desktop had a frame where ticks ≤ `confirmed` held
lines that later changed in 22 of 59 runs (70 frames in all, 1–3 a run). The window is when the two machines'
clocks part, which is what uneven load does; worker.js reads its lines the same way, so either side could post or
take a prediction and the test then fails "tick t: the desktop … and the browser …" though both rode alike.

Fix: `Rollback::settled`, set after every successful `advance_frame` to ggrs' confirmed frame as of that call
(capped at the last tick run), i.e. the last tick run on every seat's own pads with its re-runs done.
`Netplay::settled()` exposes it, web.rs's `Online.state()` carries it as `settled`, and netplay_browser.rs,
web/worker.js (its `ticks` post) and tests/web/peer.mjs wait on it instead of `confirmed`.

**Race 2, found and removed: the page's module fetch failed, and nobody heard.** With the browser alone starved,
about 1 run in 60–150 hung at start-up: page.mjs had navigated and then no state came for two minutes ("nothing new
for two minutes; the desktop: Waiting for riders.", the message the P213 fixer paraphrased). The diagnostics below
showed why: the worker's `import()` of the module failed ("TypeError: Failed to fetch", "Failed to fetch dynamically
imported module …/pkg/openmx_web.js", "WebAssembly compilation aborted: Network error: Response body loading was
aborted") and worker.js puts that only into #status, which the test never read. Two causes, both removed:

- **web.sh --check swapped pkg/ on every call.** Each test of the binary calls it (`built()`), and it ran bindgen
  again and did `rm -rf pkg; mv pkg.new pkg` even for an unchanged module, so a page loading in one test could find
  no pkg/ while the next test's --check ran. A scratch serve loop (node fetching pkg/ while --check ran in a loop)
  saw 6 404s in 23 swaps. Now --check stamps pkg/ with the module's cksum and the bindgen version and leaves an
  unchanged pkg/ alone: 0 errors over 175 runs of the same loop.
- **This machine's chromium loads its own extensions into every profile**, even a fresh scratch one: a policy
  force-installed one (/etc/chromium/policies, fetched from the Web Store a few seconds after start) and three
  `--load-extension` ones from ~/.config/chromium-flags.conf. With pkg/ untouched, failures went on: page.mjs's
  server (logging scratch-only) saw the glue request never arrive, or the wasm response's connection reset
  mid-body, a few seconds in, when the extension arrived. page.mjs now starts chromium with `--disable-extensions
  --disable-component-extensions-with-background-pages --disable-background-networking`.

What stays in page.mjs, so the next start-up hang says where it is (stderr only, not counted as progress):
- `page chromium` / `page opened` on stdout (real start-up steps, each progress to the test), echoed in its log;
- while the page has no state, every 5 s: `page: no state yet, at [readyState, body data-state, #status text]`;
- `Runtime.evaluate` with no reply in 10 s says so and the loop asks again, instead of waiting for ever.

Ruled out on the way: the service worker (0/60 failures with it, 2/60 with sw.js refused), the http keep-alive
close race (120/120 chromium fetches after ~6 s idle gaps fine), and a CDP network tap (dropped: it changed the
timing enough to hide the failure).

## What the original does

Not applicable: test harness and netplay plumbing, no game logic.

## How it was proved

- Race 1: the scratch instrumentation above (counted frames with `confirmed` past `settled` whose lines later
  changed: 70 over 22 of 59 runs). After the fix every throttled run passed (12/12 at 20–65 %, 36/36 four at a
  time at 30–70 %).
- Race 2: batches of 150 runs, six at a time, browser-only quota 15–60 %: before the extension flags 1–2 of 150
  hung each batch (with the pkg/ stamp already in); with `--disable-extensions …` 150/150, and again 150/150 with
  `--disable-background-networking` added (the committed set).
- The whole binary (`cargo test -p openmx --test netplay_browser`) unthrottled after the scratch code came out:
  20/20.
- `./kit check --all` (the gate) at the end.

## Not verified / not 1:1

- S149's actual failure message: its gate log is gone, so I cannot say which of the two races it hit.
- Race 1 was never caught exactly at the break (tick 300) in my runs, only earlier in the ride (ticks 16–75,
  while time sync settles); the fix closes it at any tick, but I have no run where the old test failed on it.
- How chromium's extension install aborts the page's fetch is not traced inside chromium: inferred from requests
  that never reached the server / connections reset a few seconds in, and from 300/300 after the flags against
  1–2 hangs in 150 before. At that base rate 300 clean runs is good but not proof.
- The four other chromium launchers (touch.mjs, chromium.mjs, gpu.mjs, shots.mjs) have the same exposure and are
  not changed here: filed as K9.
- Other readers of `confirmed()` that compare lines or hashes across machines (online.rs `summary()`/`settled()`,
  tests/netplay.rs, tests/markers_online.rs) have the same race; filed as K8, not changed here.

## Next

(none)
