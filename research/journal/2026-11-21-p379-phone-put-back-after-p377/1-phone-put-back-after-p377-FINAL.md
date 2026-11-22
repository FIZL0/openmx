# 1 — phone put back after p377 (FINAL)

Task P379. 2026-11-21.

## What I did

- The phone had build 5260, which does not have P377 (`1b763cc59` is not an ancestor of `4fce2110e`).
  The v0.19.0 release (`3d1322f`, build 5648) has it. Its `openmx-ios.ipa` is unsigned.
- Installed it with iFizBox's `tools/ui-15.sh P379 install` on omarchy-desktop. That signs with the rig's
  ad hoc profile and keeps the bundle ID: the app came up as `io.github.fizl0.openmx`, `v0.19.0 · 3d1322f`,
  with its Documents (the ISO) kept.
- Killed the app for a fresh log. Title → Race → Los Angeles (sx01) → Start Race: 3 laps, 4 riders.
- Rode it with two touch pointers in one WebDriverAgent `/actions` call (as P348): GAS held 150 s, the
  touch stick thrown hard left or right for 2.5 s every 7 s to crash. Lock given back after.
- Pulled `Documents/openmx-log.txt` with `ios fsync` (kept in `context/p379/ride1.txt`, 861 lines,
  186 per-second lines).

## What the log says

- **9 wrecks, each followed by a put-back**, after seconds 31, 38, 48, 62, 73, 127, 132, 149 and 168.
- **Every second's worst frame is under 25 ms through all of them.** From second 2 to the end, the worst of
  any second is 21.23 ms (second 123), and every second over 20 ms is led by wait (the window's acquire
  and present, at the 60 fps pace). P377's log had 29.6 ms and 48.5 ms submit-led frames at the 73 s and
  76 s put-backs; here the seconds around each put-back read 18–21 ms.
- **No `gpu: first drawn` line at all.** P377's warm frame drew every pipeline before the ride, so no frame
  in the ride drew with one for the first time.
- **One `gpu: slow submit` line:** tick 2479 (second 83), 5.40 ms, 361 draws, 140,231 triangles, 0 ground
  parts written, 0 models uploaded, GPU memory 178 MB (as before and after). Nothing new was handed over:
  it is the busiest frame of the ride (dirt spray and chunks, 116 + 81 + 61 particles that second), and
  its frame was 20.64 ms, under the bar. Nothing to fix.
- The one second over 25 ms is the ride's first: 33.49 ms led by wait (32.81 ms, a missed refresh) right
  after the load, not near a wreck.

No code change: the log shows the fix holds and names nothing left to fix.

## What the original does

Not applicable: frame times of the phone app.

## How it was proved

- The phone log above, from a build with P377, on the iPhone 15 Pro; every per-second line read
  (`worst` over all 186 seconds, the wreck and put-back lines, and the `gpu:` lines).

## Not verified / not 1:1

- **The GPU's own time per frame is not in the log** (`gpu no frame timed` on every line), so a frame's GPU
  cost is only seen through wait. The frame budget held, so it did not matter here.
- **Wrecks came from hard steering only**, all reasons 1, 2, 3, 6 and 11 (no jump crash at speed). A wreck
  kind this ride did not make could still draw something the warm frame missed; the `first drawn` line
  would name it on a later log.
- **Ghost shadows in the sun-map pass are still not counted by `first drawn`** (P377's note). No hitch
  showed up at the put-backs, so they cost nothing measurable here.
- **The release IPA was re-signed by the rig, not by the user's own signer.** Same binary, different
  signature; it should not change frame times.

## Next

Done.
