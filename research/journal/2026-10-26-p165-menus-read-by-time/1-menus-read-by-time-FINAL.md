# 1 — menus read by time (FINAL)

Task P165. 2026-10-26.

## What I did

Why P109 was not enough: P109 made the Reader's bounce/repeat thresholds milliseconds, but it still
*counted* time as 1/60 s per read. The front end reads once per drawn frame (5–165 fps, or unpaced
with the frame limit Off), and the pause reads once per 30 Hz ride tick, so a hold repeated at a
different rate at every fps, a tap shorter than one frame at 5 fps was lost, and two taps inside a
frame folded into one. The menu's own counters (the "Player 1" card, cue/claim waits, the splash
breath, the stills) also advanced one per step.

The fix reads the hands by **when they moved**, not when we looked:

- `window::shell_us()` is one shell clock. `Stamps` maps a device's own millisecond clock onto it
  by the smallest `now − stamp` seen (js event times; X server times, calibrated by a
  PropertyNotify round trip and every key event).
- `pad.rs`: every js event is stamped; each change of the menu-relevant state (pressable buttons +
  left stick bytes) becomes a `Moment (shell µs, Report)`. Keyboard seats get the same from
  `window.key_moments()` (X key events with server time; the GPU window stamps at pump time).
- `menu.rs`: `Reader::read_timed` replays the moments by time — the 35 ms bounce, the 20 / 5 tick
  repeat in microseconds of 1/60 s — and hands back extra presses through `more()` when one read
  covers several. Devices without moments (tapes, fakes) keep the old one-read-one-tick reader.
  `Readers::read_at` does the same for the front end's joint read.
- Menu pacing: `Menu::elapse_to(now)` turns real time (capped 250 ms) into 60 Hz ticks that the next
  step takes, so the card, cue waits, splash breath and stills run on time; untimed menus take one
  tick per step as before.
- `main.rs`: front end, loading screen and ride pause/start all read timed and drain `more()`.

## What the original does

Not applicable — this is the shell's own menu input; the original runs a fixed 60 Hz frame.

## How it was proved

`crates/openmx/tests/menu.rs` (P165 section), through `./kit check -- -p openmx --test menu`
(161 passed):

- `every_menu_moves_the_same_for_the_same_hand_at_every_frame_rate`: every walked screen (splash,
  front end, each Settings/Garage tab, pause over a ride) × Down/Right (+ L1/R1 tab switches on the
  first Settings and Garage tab) × d-pad / stick / keyboard × tap, short hold, 1 s, 3 s, two quick
  taps × 5/30/60/120/165 fps, frame limit Off (jittery unpaced gaps) and Refresh. 3990 runs, the
  row/value/tab change counts and the final state equal to 60 fps's in each; a tap moves one at 60.
  Negative controls: with the old `read_hosted` on the front path it fails (Main, short hold, d-pad,
  120 fps: 3 vs 1); with the old `read_joint` on the pause it fails (Pause, stick tap, 5 fps: 0 vs 1).
- `the_player_1_card_lasts_as_long_at_every_frame_rate`: the card is up its 1.2 s ± one frame.
- `a_second_seat_s_pause_moves_the_same_at_every_frame_rate`: seat 2's pause in a split, read at
  30 Hz ticks inside frames of every rate, moves the same.

## Not verified / not 1:1

- No run on real hardware at each frame rate: the tests drive the fake pad/key paths (FakePad js
  events with stamps, synthetic key moments) through the same Reader/Menu code main.rs uses; the
  main.rs wiring itself is mirrored in the test rig, not executed.
- GPU (winit) window: keys have no event timestamps, so they are stamped when the event loop pumps —
  frame-quantised again (two taps inside one frame still read as two, but their spacing is the
  frame's). The X window path is exact to the server's millisecond.
- Clock offsets are estimated by the minimum observed gap (ms stamps): an edge can land up to ~1 ms
  off, which could move a repeat that sits exactly on a threshold.
- Keyboard-driven stick ramp (the WASD layout) and a trigger-throttle Cross still change at frame
  time; keys typed beside a seated pad are frame-time too.
- Escape (back to the front end) is still a per-frame edge.
- The loading screen reads only Back, timed; nothing else there was tested.
- Mods tab tap at 60 fps is not asserted to move one (its list can be empty in the slot).

## Next

Nothing for P165.
