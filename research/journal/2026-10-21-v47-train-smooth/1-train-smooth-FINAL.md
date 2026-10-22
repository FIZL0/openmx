# 1 — train smooth (FINAL)

Task V47. 2026-10-21.

## What I did

- **The cause.** Of the brief's four suspects, it was the first. The shell placed the path cars once a
  tick (`traffic.placed(traffic_time, …)` inside `if simulated`), 30 times a second, and every picture
  between two ticks drew that same placement. At 60 Hz each position showed for two frames, and at the
  ~680 fps an unthrottled run draws here, about twenty. The bike, cameras and plane were already blended
  between the last two ticks (P52). The other three suspects were not the cause:
  - *Keys stepped:* the train's clip is position keys only (its `Body` channel). `toy::Path` already
    reads it as the Hermite/Catmull-Rom curve V41 proved against the original to 0.01 ft, so nothing
    there steps. The rotation-kind-0 keys belong to `anim.rs`'s skinned clips, which no path car uses.
  - *Clock off the shell's frame:* the clock was already the sum of each tick's `dt`, not a frame clock.
  - *Cars sampled at different times:* all thirteen cars read the one clock and walk their lag back from it.
- **The fix** (`crates/openmx/src/main.rs`): the tick only advances `traffic_time` now, and saves the
  value from before as `traffic_before` at the same point the bike's `before` is taken. Every picture
  places the cars at `toy::between(traffic_before, traffic_time, blend)`, using the same blend the bike
  uses. The clip's curve and the lag walk are therefore evaluated at the moment the picture shows. This
  is not a slide between two matrices: a car on a bend stays on its rails, and one clock keeps the cars
  coupled. Simulation untouched: the path cars reach nothing in the ride.
- **`toy::between`** (`crates/game-render/src/toy.rs`): the drawn clock, with the module note saying so.
- Nothing was taken from opentennis: it has no scenery moving on a clip.
- **Cost:** the placement now runs every drawn frame instead of every tick. On free06 (21 cars on clips),
  a release `--timings` run at 320x224 still draws a whole frame in 1.48 ms.

## What the original does

The car update (`./kit fn 115540`, V41's notes in `research/oracle.txt`) runs once per 1/30 s scene step,
and the original draws one picture per step, so every picture it shows has a new position. It is smooth
because it never draws a frame between two updates, not because it interpolates.

## How it was proved

- **The original, live (PINE, biplane state on free06, emulator copy 2):** `./kit pine watch` on
  train_car_03's position (object +0x70, see V46's oracle note) over 120 game frames. It moves
  1.8367–1.8447 ft every game frame (30 Hz), with no repeated frame. That is 55.1 ft/s.
- **The port's shell, live (free06, `--seconds 6`, release):** a temporary per-frame print of
  train_car_01's drawn position and drawn clock, removed before the commit. Over 4,081 frames the drawn
  clock never stepped back and never held. The car moved 54.54–55.71 ft per clock-second (median 55.12;
  the spread is the print's 4-decimal rounding) with no repeated frame. That matches the original's
  55.1 ft/s.
- **Test** `crates/game-render/tests/toy.rs::the_train_moves_every_frame_drawn_and_stays_coupled`, on
  the real disc: the shell's schedule (ticks at 30 Hz, each adding its `dt`, pictures at 60, 144 and
  165 Hz at the blended clock) over five seconds from the biplane state's clock. For all thirteen cars,
  each frame's step is within 10% of the frame before (no repeat, no doubled frame). Each car's gap to
  the one ahead changes by under 0.01 ft from frame to frame and stays within 0.5 ft of its lag. As a
  control, the same frames drawn at the last tick's clock (the old way) do repeat.

## Not verified / not 1:1

- **By eye against the original in PCSX2:** not done. A session here sees still frames only
  (screenshots), and a still frame cannot show a stutter. The proof is the two frame-by-frame traces
  above and the test.
- **The coupling breathes by about 0.17 ft over five seconds** (cars 0 and 1: 56.84–57.01 ft for a
  57 ft lag). This comes from the lag walk V41 ported, which reads each segment's arc length as linear in
  clip time. It happens per tick too, and this task did not change it. Whether the original's gap breathes
  the same way was not measured live: only one car was watched.
- **The picture is a tick behind the clock**, like the bike (P52): the train is drawn where it was up to
  1/30 s ago. That is the port's render-side choice, not the original's.
- **A late tick holds the picture:** `Pacer::blend` clamps at 1, so if a tick runs late the cars wait at
  the last tick's clock, as the bike does. Not seen in the 6 s run (largest clock step 4 ms).

## Next

none
