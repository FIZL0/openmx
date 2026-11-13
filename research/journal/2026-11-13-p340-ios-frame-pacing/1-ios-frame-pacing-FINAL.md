# 1 — ios frame pacing (FINAL)

Task P340. 2026-11-13.

## What I did

- Read the user's log, `research/p340/openmx-log-iphone-2026-11-13.txt` (v0.13.0 build 4179, A17 Pro,
  Fifo, the screen set to 60 for the frame limit). While riding, the lines a second show the CPU side is
  not the problem: input+sim+stream+scene+hud come to ~1.5 ms, and `present` is ~15 ms, of which
  "waited" (the drawable acquire plus the present call) is 9–12 ms. Every second averages 60.0 fps, but
  25–37 of its 60 frames are over 16.7 ms and the widest gap is 18–24 ms (sometimes 28–55). So the frames
  go up on every refresh, but **the frame ends wander by ±3–4 ms around the 16.7 ms grid**.
- Why that matters: on iOS (`ios_gpu.rs` `draw`) the loop paces itself in `get_current_texture`, which
  blocks for a drawable. That happens *inside* the next frame's draw, after its scene has been blended
  at `moment`. And `moment` (main.rs, the loop's end) is read right after the previous `queue.present`,
  which does not wait. So the moment each picture is posed at is "previous drawable + previous frame's
  work", not the refresh it goes up on. The screen shows pictures 16.7 ms apart, but they were posed
  13–21 ms apart. The bike, rider and camera move in uneven steps on an even screen, and that is the
  stutter. (P120 had already moved the moment to the loop's end so ticks would not skew it. The display
  wait coming after the blend is the part that remained.)
- Fix: `pace::Refreshes` (pace.rs). Each moment is the last one plus a whole number of refreshes (the
  number the wall clock is nearest, at least one), nudged 1/32 of the way toward the wall clock so a
  screen slightly off its nominal rate cannot drift it. When it is more than a refresh off (a stall, the
  app away) it takes the wall clock again (a stall lands on the refresh nearest the wall clock), and it never goes back past the last moment. main.rs applies
  it to `moment` at the loop's end, only on iOS, only on the wall clock, only when the present waits
  (Fifo) and there is no `--draw-fps`. It uses the window's `refresh()` (the P248 screen rate, else the
  top rate). A missed refresh still gives a two-refresh step, so a real miss shows as one long frame and
  is not smeared.
- The phone's line a second now ends with `, pictures A-B ms apart`: the least and most step between two
  moments that second. That is the before/after number to read from the user's next iPhone log. It
  should read about 16.4–17.0 at a 60 Hz screen. Before this fix it would have read the widest-gap
  spread above (about 13–21).
- Took nothing from opentennis. Its iOS loop has no render-side blend between ticks, so it has no
  version of this problem.

## What the original does

Not applicable. This is the picture's pacing, which AGENT.md says is ours. Ticks are still owed from
`moment` by the wall-clock schedule (`Pacer`). The moment moves at most about a refresh from the wall
clock, so tick timing over a second does not change, and the simulation is untouched.

## How it was proved

Unit tests in `crates/openmx/src/pace.rs`, run with `./kit check -- -p openmx --bin openmx pace`:
- `a_phones_moments_keep_to_the_refresh`: 600 frame ends with the log's jitter (0–7 ms off a 60 Hz grid;
  the test first checks they match the log: widest > 19 ms, > 200 over 16.7). Raw steps are 13–21 ms;
  after `Refreshes` the steps are inside 16.3–17.05 ms, and every moment is within a refresh of its
  frame's end.
- `a_phones_moments_follow_a_missed_refresh_and_a_drifting_one`: a missed refresh gives a 33.3 ms step.
  A screen really at 59.7 Hz while the code is told 60 never drifts more than a refresh from the wall
  clock over 36,000 frames.
- `a_phones_moments_take_up_a_stall_and_never_go_back`: a 509 ms stall lands
  onto the refresh nearest the wall clock (within half a refresh), frames that end early never move the moment backwards, and with no refresh known the moment
  is the wall clock.

## Not verified / not 1:1

- **No measurement on the phone.** I have no iPhone here and no iOS build or run was made in this
  session, so there is no before/after frame-time number from the device yet. The new
  `pictures A-B ms apart` field in the phone's line a second is how the user's next log measures it. The
  tests prove only the timing model on synthetic frame ends built from the log's own spread.
- **Diagnosis is from the log alone.** The log records frame ends, not when pictures actually went up.
  "Every picture goes up on its refresh" is inferred: 60.0 fps average at a 60 Hz Fifo screen with three
  drawables. If iOS were really dropping refreshes in those seconds, the stutter would have a second
  cause this fix does not touch.
- **The rare 28–55 ms frames are not addressed.** For example 10 s: worst 55.5 ms; 6 s: 35.7 ms; 8 s:
  28.4 ms. Each time it is all `present` (the drawable wait), with nothing on the CPU. These are real
  missed refreshes whose cause the log does not show (GPU or compositor). Filed as a follow-up (P346).
- **Android and the desktop are left as they were.** They have the same order (the acquire wait after
  the blend), but the task and the log are iOS-only, and those builds' frame ends were not measured
  here. Filed as a follow-up (P345).
- The 1/32 nudge and the one-refresh resync limit are my choices, tuned on the synthetic test and not on
  the device.

## Next

Ask the user for a new iPhone log on a build with this change and read `pictures A-B ms apart`. Then
look at the 28–55 ms present stalls (follow-up).
