# 1 — split pause (FINAL)

Task P172. 2026-10-26.

## What I did

Three faults of a local split-screen pause (P84), each fixed and tested on its own.

- **Sound.** `audio::SeatFrame` gained `paused`, which `main.rs`'s `seat_sound` sets from
  `!seat.riding` (the seat's own pause, a pad pulled out, Start's freeze). `Sound::frame` holds a
  paused seat's voices and skips the seat entirely: no note machine, no shootoff reasons, no landing.
  When the pause closes the same voices are let go and play on from the sample they were held on.
  The hold is a second flag on the mixer's voice (`Mixer::hold` / `Mixer::held`, `Voice::held`), kept
  separate from `Mixer::pause`. That one is the engine note's own: it holds the free rev under the
  limiter's loop. With the two apart, a seat paused at the limiter comes back with its rev still
  held. `SeatSound::voices` lists everything the seat owns: the engine's samples and limiter loop,
  the plane's loops, the shootoff rumble and boom, and the wreck one-shots, which it now keeps.
  Before this, a paused seat kept sending its frozen hands, so its engine sounded for everyone on
  one stuck note.
- **Camera.** `chase::PauseHold` (new section at the end of chase.rs) decides each frame between
  Riding, Held and Reframing. When Held, `main.rs` pushes the seat's camera from the tick before
  (`before_cameras[i]`) and skips the camera's whole frame: follow update, look-around on the pad
  that is driving the menu, look-back, preset cycling and the wreck switch. The camera's state is
  untouched, so Resume carries on from it. The one exception is P82's Camera page: it shows a change
  as it is made, so once the player changes their framing during this pause the camera frames on the
  frozen bike (Reframing). The pad still turns nothing.
- **Hints.** `Pen::hints` drew its slanted tab to `right() + 4` ems, so it ran 4 to 7 ems past the
  frame. On the GPU path a split view's menu sprites are laid out at the view's size and moved to
  its place without clipping, so the tab spilled into the next view (player 2's in side by side).
  The tab is now `ink::tab`, the same slanted fill but with its right end square at the frame's edge.
  The hints also shrink to fit a view too narrow for them. On a full screen it looks as before, since
  the part that is gone was off the screen anyway.

opentennis's pause is global (Bevy states). It has no per-seat pause in a split, so nothing was taken
from it.

## What the original does

The original never pauses one view of two: its pause is the whole game's. This is the port's own
P84 behaviour, so there is nothing to match. These are presentation and front-end rules (AGENT.md:
"Presentation: feels right", "Menus: yours to design").

## How it was proved

- `crates/openmx/tests/audio.rs` `a_seats_pause_holds_its_engine_and_the_others_play_on`: two seats.
  1P idles, 2P is on the throttle, then 1P pauses for 45 ticks with the mixer rendering every tick.
  The test checks:
  - every one of 1P's voices is held, keeps its exact progress, and the note stays the same;
  - none of 2P's voices is held, and they move on;
  - after Resume, 1P's voices are let go and are within a tick of where they were held, not started
    over.
- `crates/openmx/tests/right_stick_camera.rs` `a_seats_pause_holds_its_camera_and_resume_carries_on`:
  three seats on the stand-in bike. One pauses for 60 ticks; while paused, its pad is polled with the
  right stick full right.
  - Its view is bit-identical on every paused frame, and 2P's view moves.
  - For 20 ticks after Resume its views are bit-for-bit the views of a seat that never paused.
  - A control seat proves the camera was still settling, so the hold is not trivially true.
  - It also covers PauseHold's Reframing on a framing change.
- `crates/openmx/tests/menu.rs` `a_seats_pause_stays_in_its_own_view`: a 1920×1080 and a 640×448
  window, 2 to 8 players, both splits, every seat, with both the pause's own hints and its Quit
  confirm's. The seat's pause is laid out as the GPU path does it (view size, then moved to the
  view's place), and every sprite corner, hint tab included, must lie inside that seat's view rect:
  516 pauses. It failed before the fix ("sprite 208 of 240 has a corner at 2005.9,443.0, outside
  its view"). `the_gpu_draws_every_screen_as_the_cpu_does` still passes, so the CPU raster draws the
  new tab as the GPU does.
- The shell itself: `openmx --headless --players 2 --screen pause --frames 90 --trace-camera` traced
  camera 1 on all 90 frames. Camera 0 (1P, paused) traced once, on its first frame, which has no view
  before it to hold, and then held.

## Not verified / not 1:1

- **No check by ear and eye with two pads.** This session runs unattended with no pads, so nobody
  listened to the held engine or watched the held camera in a real two-pad ride. The tests above
  cover the logic, and the headless run covers the camera wiring in main.rs, but not the sound
  wiring in a live ride.
- **No wheel or wind loops exist in the port.** The brief lists "wheel and wind loops", but
  audio.rs has none: the bike's emitters are the engine note, the wreck clank and the rider's voice,
  and the shootoff. Everything that does exist is held.
- **The crowd's "oh" and cheer are not held.** They are the whole ride's (`Bus::Crowd`), not a
  seat's, so they play on when 1P pauses after a wreck.
- **A held voice cuts dead.** The hold stops mixing at once, without the mixer's 5 ms ease, just as
  the limiter's `Mixer::pause` already does. A faint click at the pause is possible. Not heard.
- **Everyone paused is still quiet.** When every seat is paused, nobody is riding, and the old
  P42/P84 path (`Sound::quiet`) still stops everything, so engines start again from the idle. This
  task covers a pause while others ride on.
- **The Camera page still moves a paused camera.** A Camera page change under the pause frames the
  frozen bike (P82 wants changes shown live), so from then until Resume that camera is not frozen.
  A pause with no framing change is frozen exactly.
- **The software renderer's split pause** (`art.draw` into the view's own frame) already clipped to
  its view. Only the GPU path's sprites could leave it, and that is the path the test lays out.

## Next

Nothing; done.
