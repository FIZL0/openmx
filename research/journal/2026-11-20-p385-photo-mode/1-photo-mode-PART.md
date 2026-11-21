# 1 — photo mode (PART)

Task P385. 2026-11-20.

## What I did

- The pause has a "Photo Mode" row, over Tricks (Tricks keeps its place counted up from Quit). Every
  ride but an online session has it. It opens only on a ride of one rider; with more it says why.
- Photo mode is a menu screen (`Screen::Photo`) over the pause. The pause already holds the sim, so
  the ride stays frozen. The shell draws the bike solid (not the pause's ghost) while photo mode is up.
- Free camera (new `crates/openmx/src/photo.rs`), starting from the chase camera's eye and look:
  - left stick moves, right stick looks, L2/R2 rise and sink
  - d-pad left/right roll, up/down field of view (10°–120°)
  - L1 held orbits the point the chase camera looked at; R1 held goes fast; Square resets
- Roll: game_render::world has a view roll (`set_view_roll`, `view_up`). Both view-matrix builders
  (software and GPU) use it. At roll 0 the up vector is exactly (0,1,0), so nothing else changes.
- HUD, FPS and the menu's own sprites are hidden in photo mode. Triangle also hides the controls card.
- Cross takes a picture: one offscreen frame at the window size, without menu sprites, written to
  `<markers dir>/photos/photo-NNNN.png` (first free number). The shell prints the path and size and
  the menu shows it as a note.
- `--photo-at N` presses Cross on frame N, for tests.
- Start resumes the ride; Circle goes back to the pause.

## What the original does

Nothing. The original has no photo mode. MXA's photo mode was the reference for the feature list only;
no code or data from it.

## How it was proved

- `crates/openmx/tests/photo_mode.rs`: the shell, headless, opens `--screen photo` and takes a
  picture on frame 30 of 40, at 640x448 and at 960x540. The ridden seat's sim tick on the "frozen"
  line equals the tick on the picture line, while 20+ engine ticks went by between them. The PNG is
  photo-0001.png and its header size is the window size. Passes, not skipped.
- I looked at the picture: no HUD, no menu, solid bike.
- `crates/openmx/tests/menu.rs` `the_pause_opens_photo_mode`: the row, its place, opening, Triangle,
  Circle, Start, Cross asking for a shot, no row online, and the note with two riders.
- photo.rs unit tests (6): start from the ride camera, stick flies along the view, orbit keeps the
  distance, roll and zoom held in range and reset, pad mapping, next free file name.

## Not verified / not 1:1

- The free camera's moves (stick, orbit, roll, FOV) are proved by unit tests only, not on a real pad
  in a window: this run is unattended and the desktop has no pad.
- Roll is one renderer-wide value. That is why photo mode opens on a ride of one rider only (one view).
- Billboards, particles and sprites drawn facing the camera may not roll with the view: not checked.
- The seat's state hash changes under the pause, because the pad is polled each frame. So the test
  proves the sim tick count holds, not that every byte of the state does.
- Race AI riders under the pause: not checked whether anything of theirs moves; only the ridden
  seat's tick count is proved.
- The task says "from the pause (and the replay, if there is one)". The shell has no replay view, so
  photo mode opens from the pause only.
- The free camera reads the pad's sticks as they are. A remapped stick scheme (dual-stick, plane)
  is not applied to it.
- In a window on the GPU path, the frame that takes the picture is drawn without the menu sprites,
  so the controls card blinks off for one frame. Not seen on screen (headless only).
- The free camera has no collision; it can fly into the ground. Not asked for.
- MXA's auto-shots (wrecks, tricks, results) are not done. The brief names them as MXA's, not as
  asked, so they are filed as P390.
- The brief's files (menu_pages.rs, game-sim camera.rs, chase.rs) are untouched. The pause rows are
  in menu.rs, and the free camera only overrides the shell's camera, so the sim camera is not changed.

## Spec review

A fresh reviewer read only the diff and the brief. It confirmed no bugs. I checked its three risks:
- The free camera's pad list is filled every frame from the raw pad, not only on the Tricks page.
- With `paused` false and `riding` false, only the bike's alpha reads `paused` alone, which is the point.
- `view_up` is used only by the two view-matrix builders.
I kept what it called "not asked", each needed by the brief:
- the global roll, for "roll"
- the one-rider limit, because that roll is renderer-wide
- `--photo-at`, for the test
- Triangle hiding the card, for a clean view

## Next

Nothing: the task lands with this.
