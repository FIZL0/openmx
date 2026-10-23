# 1 — preload meter fill (FINAL)

Task V53. 2026-10-22.

## What I did

- Followed what feeds the meter movie's `preloadMeter` variable (V28 had left it as a guess), captured
  the variable beside the rider's preload fields over recording2 played back on the original, and
  filmed the same playback with PCSX2's Video Capture to see what the movie draws for each value.
- `crates/openmx/src/overlay.rs`: `preload_fill` (the guess: force share × timer, only while firing)
  is gone. `PreloadMeter` is the speedometer's update plus the movie, stepped once a tick:
  `value` (-1 / 0 / 1..=20), `latched`, and the level drawn. `PreloadSense::of` reads the port's
  `Air` (ready, firing, force), `MaxPreload` and whether the rider is down. `Gauges.preload` is now
  `Option<f32>`: `None` dark, `Some(fill)` lit. The capsule draws lit-empty brown inside while a
  preload is possible, and the charge colour is the one measured off the original's frames
  (206,109,0, where V28 guessed 255,153,0). `Drawn.preload_lit` added.
- `crates/openmx/src/main.rs`: each `Rig` has a `preload_meter`, stepped right after the tick's
  simulation (next to `traffic_time`) from `preload_sense(seat)`, which feeds nothing on the plane.
  `gauges()` reads `rig.preload_meter.shown()`.
- Capture spec and job script: `capture/preload-meter.toml`, `capture/play_with_video.sh` (C6's
  play_capture.sh plus P49's Video Capture press, run as
  `./kit pcsx2 with -- bash …/play_with_video.sh PID context/recordings/recording2.p2m2 2374 1894 OUT.csv`,
  with the AT-SPI env from C6's notes). The captures are in `context/capture/v53/` (git-ignored).
- Taken from opentennis: nothing; it has no equivalent meter.

## What the original does

- `./kit fn 0x002abcf0` (speedo_update) binds the movie variable "preloadMeter" to speedo+0x4c, which
  is a *pointer* to the variable object. Its value is the s16 at +0xe.
- `./kit dis 2abe10 2abfc0` (speedo_preload_update, called first in `./kit fn 0x002ac400`) is the only
  writer. Its inputs come through the rider's MXUIInterface component (speedo+0x3c = rider+0xfd0,
  pointers bound in `./kit fn 0x0027c220`): +0x24 → rider+0x4a0 (preload ready: the lip test),
  +0x28 → +0x4a4 (firing), +0x30 → +0x868 (a copy of the wreck reason +0x748), +0x34 → +0x494 (the
  force's y), +0x38 → +0x4c4 (MaxPreload).
  - When not latched (speedo+0x50 == 0), the value is 0 if ready, else -1.
  - When firing and not latched, it latches and computes n = cvt.w.s(0.5 + 20·(force/max)); n < 2
    becomes 1. The value is n, unless the wreck reason is set, in which case it is -1 and unlatched.
- `./kit dis 2abfc0 2ac018`: the movie's "AnimDone" event (case-blind compare) clears the latch.
- So **the original's meter does not fill while the rider pumps.** It is lit and empty on a lip and
  shows the fired shove's strength afterwards. The brief's "it fills while pumping" was a hypothesis,
  and the code and the capture both say otherwise. The port now does what the original does: lit while
  pumping, and filled from the fire.
- The movie, from the video, steps once per 30 Hz tick (every pair of 59.94 Hz frames is identical).
  It rises through the even levels below n, two a tick, then holds n for 15 ticks, then falls through
  the odd levels below n, then is empty. That is 14 + n ticks, which is exactly how long the original
  holds its latch: 24 / 21 / 19 / 17 for n = 10 / 7 / 5 / 3. Level 20 fills the capsule's inner
  height (≈84 of 480 video lines ≈ 79 layout px = 85 − 2·3 inset).

## How it was proved

- **A PINE recording compared tick by tick**: two `./kit record` captures of recording2's playback,
  1894 rows each, `context/capture/v53/recording2-preload-meter{,-2}.csv` (the variable's word, the
  latch, and the rider's ready / firing / armed / timer / force / max / wreck / stick fields).
  `tests/overlay.rs::the_preload_meter_writes_the_originals_variable` feeds each row's rider fields to
  `PreloadMeter`.
  - Every row matches the port's tick or the one before it. Rows tear between the bike's update and
    the HUD's (8 and 10 rows of 1894).
  - At least 99% of rows match on the same tick.
  - The four shoves' values (10, 7, 5, 3) are exact.
  - The port's holds are exactly 14 + n. The capture's are within one row: one file has the 7 held 20
    rows, because the row at its start tore.
  - Proved to have run, not skipped: an impossible shove count made it fail on both files.
- **Looked at beside the original's frames**: PCSX2 Video Capture of the same playback. The four
  fires were located by orange-pixel count and lined up with the capture's fire ticks (the spacing
  matches 2 frames a tick). The per-tick fill heights became
  `the_preload_meter_plays_the_originals_movie`'s literal sequences.
- `the_preload_meter_shows_the_shoves_strength`: the four captured forces → 10 / 7 / 5 / 3. The
  computation uses `ps2_float::fpu` div / mul / add / to_i32.
- `the_preload_meter_follows_the_preload`: dark, then lit and empty for five pumped ticks, the shove
  held through the movie whatever the preload does, the latch let go at 24, and a downed rider gets
  nothing.
- The port's own preload fields were already held to the original by game-sim's
  `tests/air.rs::the_preload_step_is_the_originals` (oracle), which this does not repeat.

## Not verified / not 1:1

- **The movie's phase against the variable is assumed, not seen.** The port draws level 2 on the
  fire tick itself. The video runs ~0.11 s off a PINE log (P49), so whether the original's first
  orange frame is the fire tick or the one after was not pinned. Each later level follows from that
  choice.
- **The movie's timeline is measured, not read.** `Ingame_Preloadmeter.fsh` was not decoded (there is
  no Flash player here). The rise / hold / fall rule is fitted to four values (3, 5, 7, 10). Values
  1, 2 and 11–20 follow the same rule but were never seen on the original.
- **AnimDone's condition is partly unread.** The handler also requires its fourth argument's +0xe to
  be 1. The port takes the event to come when the movie empties, which is what the captured latch
  lengths show. What that argument is was not followed.
- **Colours and shape are by eye.** The lit-empty brown (88,44,10) and the charge (206,109,0) are
  medians of PCSX2's video frames, drawn opaque. The rounded bottom of the charge and the movie's
  partial-pixel level 1 (3 px, not 4) are not reproduced.
- **Not ridden end to end in the port.** No test rides a port bike up a lip and checks the meter
  from the port's own sim. The meter is fed captured fields, and the port's preload step is proven
  separately by oracle. A port-vs-capture run of the whole sim was not attempted: two playbacks of the
  same recording already fire a tick apart (3284 vs 3285), so cross-run tick comparisons are not
  sound.
- **Online / split views:** the meter steps once per simulated tick for every seat, offline and
  online. Online rollbacks re-simulate inside the session, and the meter sees only the confirmed
  tick, which is presentation, not simulation. The original draws no meter in a split view (V28), but
  V42 put one in every view. That V42 choice is kept.
- The port's sense of "wrecked" is `state.wreck.is_some()`. The original's +0x868 is copied from
  +0x748 at a point in the frame that was not followed, so the tick it changes may differ by one.

## Next

none — task complete.
