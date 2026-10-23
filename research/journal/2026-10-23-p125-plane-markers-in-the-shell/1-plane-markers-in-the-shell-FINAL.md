# 1 — plane markers in the shell (FINAL)

Task P125. 2026-10-23.

## What I did

- **I reproduced it in the real shell, not in a test copy.** I ran `openmx --headless --vehicle biplane --play`
  on a hand-written tape: 300 ticks of gas, Down held 15 ticks, 90 ticks of gas, Up held 15 ticks. I
  added a temporary per-tick trace to `Seat::pass` (each d-pad direction's held/prev_held_ms/held_ms,
  the flight's crash reason and pose, the plane's drawn `at`, and the `Did`). I removed it before the commit.
  - On the plane, **Up fires and the put-back takes effect.** Down crossed at 200 ms and printed
    `marker: placed in slot 1`. Up crossed and printed `marker: to slot 1`. The flight and the drawn plane
    were at the placed spot (z 1573.17) that tick, and the plane flew on from there. The plane hash is
    unchanged before and after the refactor below (0xa4df06f9ab7f7860).
  - **What the user saw was the camera.** `--trace-picture` and shots before and after the Up: the plane's
    chase camera trails a flying plane by many ticks, and nothing told it the plane had jumped. On the tick
    after the Up, the eye was still about 300 ft from the plane (z 1275 while the plane was at 1573),
    pointed away from it, and the frame showed empty terrain. The camera then flew back across. The plane
    was off the screen for most of a second, and longer after a longer flight away. The Up looked like it
    did nothing.
  - The leads in the brief were checked and ruled out:
    - `riding = &[]`: offline every seat is in the ride, the same as the bike's `markers::frame`.
    - `Table::SHIPPED` vs the seat's table: every preset binds the d-pad's commands alike (`bindings::FIXED`).
      The layouts (`pad::Scheme::ride`, `PlaneScheme::fly`) do not touch the d-pad.
    - The 200 ms crossing works on the plane's poll.
    - The plane's frame does go through `Seat::pass`.
    - Nothing in the shell undoes the put-back.
  - On the first tape run Down and Up were both refused ("nothing on slot 1"), because 30 s of gas had
    wrecked the plane. That is the designed refusal (a wrecked plane places nothing), not this bug.
- **The fix, `main.rs` camera block + `chase::follow_jump` / `chase::carry` + `pace::jump`:**
  - If the flown plane moves further in one simulated tick than it flies (`pace::jump`: P120's `jumped`
    test with speed × dt as the move it was making), the camera goes with it.
  - A flying plane **carries** the camera: the look-at and eye lags are shifted by the jump. The plane
    stays where it was on the screen, and the camera then swings round behind it as after any turn.
  - A standing plane (under 1 ft/s, so landed by another player) **snaps** the camera, as V29's reset does.
  - A plain snap was tried first for the flying case. It loads the lags with the unlagged eye, which in
    flight sits ~6 ft behind and 16 ft above the plane's centre, so the plane was below the frame. Shot
    checked and rejected.
- **A second bug found on the way, `Garage::markers`:** an Up onto another player landed the plane (`land`
  → `Plane::reset`), but the flight was only reset onto the ground at the *next* step. For one tick the
  camera and the picture read the flight the plane had left, and the camera snapped onto that. The flight
  is now reset, `placed` and posed on the tick it lands, as the online `lane_markers` already did. This
  only moves the same `Flight::reset` call from the start of the next step to the end of this one, so the
  flown result is the same.
- **`Garage::pass` (vehicle.rs)** is now `Seat::pass`'s machine step plus the up machine's markers, moved
  out of main.rs, so the tests run the shell's own code instead of a copy of it.
  `tests/markers.rs`'s `pass` helper now calls it, so the P107 tests go through it too.
- **Tests (`crates/openmx/tests/markers.rs`):**
  - `the_shells_plane_places_and_puts_back_in_view_under_both_layouts`, under the New and Original
    layouts:
    - The pad's report goes through `pad::Scheme::ride` and `PlaneScheme::fly`, with R2 or Cross as the gas.
    - 30 s of flight, then a held Down places slot 1. The plane flies 649 ft away and a held Up puts it back.
    - The flight equals the kept one, and the plane is drawn at the mark.
    - The plane's camera is the shell's, built as `main.rs` builds it on the disc's presets. It shows the
      plane on the Up tick and on each of the 60 ticks after (under 150 ft and under 0.5 rad off the middle).
    - The same tick filmed without the carry does not show it, which proves the failure this fixes.
  - `in_a_split_left_picks_the_other_player_and_up_lands_by_them`: two players, the first on the plane and
    the second on the bike. Left steps to `Pick::Player(1)`, a held Up answers `Teleported(Player(1))`,
    and the plane stands landed within 1 ft of the other bike, in view.
  - Both tests were made to panic at their end once, to show they ran on the disc and did not skip
    (`P125 New/Original: placed, flown 649 ft away, put back and filmed, (27.1 ft, 0.24 rad)`;
    split `(17.2 ft, 0.27 rad)`).
- **P124 (the bar on the plane) is not done here**, although the brief says "do it with this": P124 is
  open and in progress in slot s1 at the same time. Doing it here would have been two copies of the same
  change. My tests sit before the P107 section of `tests/markers.rs` rather than at its end, where s1's
  P124 tests are appended, to keep the merge clean.
- From opentennis I took nothing: it has no markers or chase camera teleport.

## What the original does

The original has no session markers and never teleports the plane. Its only plane jump is the reset (the
chord, a wreck's recovery), which snaps the plane camera (V29, `tests/biplane_camera.rs`). Carrying the
camera with a flying plane's jump is this port's own presentation.

## How it was proved

- The real shell, headless, on a hand-written 30 Hz tape, traced tick by tick: Up fires on the plane and
  the put-back takes effect in the sim, and the camera's eye after it (`--trace-picture`) was 300 ft away.
- Shots of the same tape at ticks 410, 414, 430 and 470 (320×224), before and after the fix: before, the
  plane was off the screen after the Up; after, it is in the middle of the frame the tick after the Up.
- The two tests above, through `Garage::pass` (the shell's own pass) under both layouts, on the disc's
  biplane and its camera presets, including the uncarried camera failing on the same tick.

## Not verified / not 1:1

- **The real pad and keyboard were not driven.** There is no pad here, keystrokes do not reach the window,
  and the user's own d-pad cannot be pressed. The tape and the tests feed reports in at the same point as
  the live pad, after `pad::Scheme::ride` / `PlaneScheme::fly`. But the device layer (evdev/keyboard →
  report, including the d-pad's pressure byte) was not exercised for Up. Down works for the user on that
  path, and Up's report bits come from the same code.
- **Online was not run.** The camera carry reads the plane's jump between `before_planes` and the
  garage's plane after `Garage::follow`, so it should cover an online seat's `lane_markers` put-back too.
  No online ride with a plane marker was flown to see it.
- **The user's exact session was not reproduced.** That includes the front end's menu flow, their layout,
  preset and scheme, the track, and how far they flew from the marker. If the camera is not the whole of
  what they saw, this does not fix the rest.
- **The carried camera is presentation and has no original to match:** the original never teleports a
  flying plane. The 1 ft/s "standing" threshold that picks snap over carry is this port's choice.
- **The bike's marker teleports do not carry or snap the camera either** (unchanged). Its camera trails
  closely enough that the user has not reported it. A long Up on the bike will swing the camera across
  the map in the same way.
- **The split test was not run in the windowed split shell.** It drives the shell's own `Garage::pass`
  and spots for two seats but not `World::simulate` itself.
- **The bar on the plane (P124) is left to P124**, in progress in s1.

## Next

Nothing for this task. Once P124 lands, the user can check the plane's markers by eye: the bar, then
Down, fly away, and Up.
