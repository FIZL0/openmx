# 1 — browser guest loses the host while loading (FINAL)

Task K33. 2026-11-04.

## What I did

`two_pages_host_and_join_from_the_online_menu` (crates/openmx/tests/netplay_browser.rs, driver
crates/openmx/tests/web/online.mjs) failed under load in F42's merge gate and passed alone. F42's gate log had been
overwritten, so I reproduced the failure by stress instead. I ran 10 copies of the built test binary at once
(`netplay_browser-… two_pages --nocapture`) beside 32–64 busy loops that I started and killed by their own pids. Before
the fix, 9 of 10 failed. Each failure had one of three causes, and each cause was a race rather than a timeout that was
too short:

1. **The guest's lobby took the host as gone while a page loaded the world (the main one).** In a browser, the lobby's
   network task (openmx-net `join`) runs on the shell's own worker thread. Loading the world is synchronous and took
   7–14 s under that load. The host page sends no pings while it loads, and the guest page cannot read any while it
   loads. The guest's `HOST_GONE` (6 s) fired, the N11 handover made the guest the room's host, and the two pages rode
   apart. The hashes then diverged with "0 desyncs". On desktop the network has threads of its own, which is why only
   the browser was affected. The ride layer already allows for this (`recover::LOADING` = 60 s); the lobby did not.
   The fix is in `crates/openmx-net/src/net.rs` `join()`:
   - From `Go`/`DropIn` until the host's ride is first heard (a datagram or a `Ride` message), the guest waits
     `HOST_LOADING` (60 s, the same allowance as `recover::LOADING`) rather than `HOST_GONE`.
   - A host-gone wait that wakes more than `LATE` (1 s) past its deadline did so because this thread was not running
     (the guest's own load, or a hidden tab). In that case the wait starts again, so the messages that arrived in the
     meantime are read before the host is taken as gone.
2. **The room code was read before its console line arrived.** The page's worker PUTs the code straight to the
   driver's server, and its console line comes through DevTools afterwards. Under load the PUT came first, and
   `host.lines…at(-1)` found no code ("host: no room code (undefined)"). online.mjs now waits for the
   `lobby: room CODE …` line, which the page prints after the put.
3. **Profile removal threw on a ride that had passed.** Chromium's helper processes (GPU, network, storage) outlive
   the browser process for a moment and keep writing into the profile. `rmSync` on the browser's exit hit ENOTEMPTY,
   and node exited 1. Chromium now runs in its own process group (`detached`). On quit it is sent `Browser.close`; the
   driver waits up to 10 s for the whole group, SIGKILLs only what remains, and then removes the profile. A removal
   that still fails is logged rather than thrown, and the test removes its scratch dir afterwards anyway.

No timeout in the test or driver was widened.

## What the original does

Not applicable: this is test and netplay infrastructure, not ported game code.

## How it was proved

- Stress, the same as the reproduction: 10 concurrent runs of the test binary.
  - Before the fix: 9 of 10 failed, with world loads of 7.3–13.7 s.
  - After the fix, with 32 busy loops: 10/10 passed, with loads of 5–9 s.
  - After the fix, with 64 busy loops: 10/10 passed, with loads up to 10.9 s.
- `./kit check` pieces were green:
  - `-p openmx-net`: 25 tests, including the host-goes-silent handover test.
  - `-p openmx --test netplay_browser`: 22.
  - `--test netplay`: 61.
  - `--test lobby`: 79.
- The full gate (`./kit check --all`) ran through `./kit done`.

## Not verified / not 1:1

- **F42's own failure log is gone.** Its gate log was overwritten, so the failure was reproduced by stress rather than
  matched line by line to that log. All three causes were seen under stress. Which of them hit F42 is not known.
- **Detection is slower for a host that dies while loading.** A guest now takes up to 60 s, instead of 6 s, to notice
  a host that crashes after `Go` and before its first datagram. This was chosen to match `recover::LOADING`; it is not
  tested.
- **The `LATE` re-arm is untested outside the load case.** It was exercised only by world loads. These were not
  tested:
  - a host that stalls in the middle of a ride, in a browser;
  - a stall longer than iroh's QUIC idle timeout, where the connection itself may drop, so the re-arm cannot save it.
- **page.mjs has the same pattern.** It still removes its profile with a bare `rmSync` on exit, the pattern that threw
  here. It was left alone because it is not this test. Filed as a follow-up.
- **Desktop behaviour is changed only by the 60 s allowance.** Desktop threads never wake late, so `LATE` should never
  fire there, but no desktop test was made to show it.
- **The real merge gate's load was not reproduced.** The busy-loop stress is my stand-in for it.

## Next

None. The task lands with this entry.
