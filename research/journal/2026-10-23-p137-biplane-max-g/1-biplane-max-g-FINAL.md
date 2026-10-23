# 1 — biplane max g (FINAL)

Task P137. 2026-10-23.

## What I did

- `Knob::PlaneMaxG` ("Biplane max G", word `plane_max_g`). It is per seat and sits on the menu after
  Biplane Turn. Its values are Off (stored as `ORIGINAL` = 100, the default) or 3–9 g, a g a step, round a
  ring: 9 g up is Off and Off up is 3 g; 3 g down is Off and Off down is 9 g. A press from the default must
  change the row (tests/menu.rs presses every row). `Mods::set` clamps the value into 3..=9 unless it is Off.
  `Mods::parse` rejects numbers outside that range.
- `Mods::plane` writes the limit into `phy::Autopilot::max_g`, with 0 meaning off. It also writes the roll
  floor `max_g_roll`, which is 1 / (the top-speed factor of PlaneSpeed × PlaneTopSpeed × AirDrag^(-1/3)),
  capped at 1. Neither one is a file key. `Flight` copies both in `new`, `retune` and `put_back`, so
  settings, presets, tapes, `Mods::replane`, P98's retune and the markers all carry the knob. The snapshot
  wire carries both. The hash walk adds them only when the limit is on, so Off hashes the same as before.
- The limiter (`biplane::Aero::g_limit`, called from `Flight::hold_g` every substep when the limit is on):
  - **Elevator.** It solves for the angle of attack at which the lift is AIM (0.98) × max_g × weight, in each
    direction. The bound is the elevator that trims at that angle, plus a lead of LEAD (10) × (the limit
    angle − where the angle is headed). "Headed" means the angle AHEAD (0.08 s) on, using the angle's
    current rate (pitch rate minus flight-path rate) and the rate at which the limit angle moves as the
    speed changes. On a fast plane the look-ahead is shortened so the lead's damping stays under DAMPEST
    (1.4) of a substep. `elevator_command` and the PI's `pitch_integral` are clamped to the bound, which
    covers both the File and the New schemes.
  - **Free side.** A side is left free when its limit angle is past the lift curve's peak, or when holding
    that angle would need more elevator than the surface has. The stock plane at stock speed hits this, so
    the limiter never acts there.
  - **Angle of attack.** The angle is recomputed fresh each substep from the body's velocity, through the
    same acos table the aerodynamics use. The frame-old `measure` angle overshot. Without the table, the
    small angles came out wrong at 3 g, because the table is linear from 1, so below about 0.0115 rad the
    angle it gives is ≈ α²/0.0115.
  - **Ailerons.** Above the corner speed the aileron is scaled by corner / speed, but never below
    `max_g_roll`.
- The menu row shows the value as `6 g` or `Off`, and its note says "Off is the original game". It steps
  with `Knob::step`. The Key::Knob handler now reads the value from the settings instead of parsing the
  row's text.

## What the original does

It has no g limit at all. This is a new modifier. With the knob Off, the original's plane flies untouched.

## How it was proved

All of this is in `crates/game-sim/tests/plane_max_g.rs`, run against the disc's plane (biplane.phy):

- **`a_fast_pull_never_passes_the_limit`.** Planes at 300% and 200% top speed, at every limit from 3 to 9 g,
  flying the File and New schemes. Each makes a full pull, a full push and a knife-edge pull from level
  cruise at that plane's top speed, for 6 s. No frame goes past the limit. The pull's peak also reaches at
  least 92% of the limit, so the limiter is not just sitting on the stick. Off, the 300% plane pulls more
  than 15 g.
- **`a_fast_roll_is_the_stock_planes`.** With the limit held, full-aileron roll at 300% stays within
  0.8–1.25× of the stock plane's roll at its top speed. Measured: 141 deg/s stock, 169 deg/s held, 505 deg/s
  Off.
- **`the_stock_plane_is_never_held_at_9g`.** The stock plane flies a 20 s cruise plus 8 s each of pull, push,
  roll and knife-edge. With 9 g it hashes the same as Off on every frame, so the limiter never acts there.
- **`off_flies_the_original`.** Off leaves the plane's numbers the file's. The hash walk has the same leaves
  and no max_g leaf.
- **`a_marker_and_a_retune_keep_the_limit`.** replane at 6 g → max_g 6 with roll ≈ 1/3; put_back keeps it;
  replane at Off → (0, 1); a retune there and back hashes the same as `Flight::new`.
- **`the_knob_steps_and_keeps`.** Covers step, shown, least, most, text and parse.

Tuning sweep (temporary, removed). "Worst" is the largest |g| / limit over every scheme, manoeuvre and limit:

| plane speed | worst  |
|-------------|--------|
| 150%        | 0.9897 |
| 300%        | 0.9906 |
| 400%        | 0.9910 |
| 450%        | 0.9957 |
| 500%        | 0.9966 |

Stock stays bit-equal at 9 g. A LEAD of 8 broke stock bit-equality on a New-scheme push, and an AIM of 0.99
passed the limit at 500%.

## Not verified / not 1:1

- **No PCSX2 proof.** The original has no such modifier, so there is nothing to compare against. Off is
  proved to be the port's existing plane, unchanged.
- **The aileron ease is linear (corner / speed), not the brief's (corner/speed)^2.** I measured the roll
  damping going with speed, so the steady full-aileron rate goes with speed. Squaring the factor would make a
  fast plane roll slower than the corner speed's.
- **The ease has a floor of 1 / (top-speed factor), not the corner alone.** At high limits (around 9 g) the
  corner speed (≈140 ft/s) is below the stock top speed (156.7). Scaling by the corner alone would ease the
  stock plane and break "stock is never touched". With the floor, the stock-speed plane is never eased, and
  the 300% plane rolls at about the stock plane's top-speed rate instead of the corner speed's.
- **The ailerons are eased even when the elevator side is free.** The corner math assumes full elevator. A
  plane that can never pull the limit at full elevator still gets its roll eased above the corner, but only
  if it has also been sped up by a speed knob, because of the floor.
- **A held pull settles about 2–3% under the set g.** It peaks at 97–98% of the limit at 300%, and 92% is the
  test's floor, because AIM is 0.98. That margin covers the leftover lag of the look-ahead, which models
  speed change and path rate only approximately.
- **g is measured per frame, not per substep.** The tests read the load from the last substep of each 30 Hz
  frame. A substep between frames could peak a little higher. The aim's margin should cover it, but this is
  not proved.
- **The limiter's model is approximate.** The fresh angle of attack ignores the sideslip's share in a knife
  edge, apart from what `flow_angle` does with it. The flight-path rate uses the last substep's lift. The
  speed rate is thrust minus drag on the pressure minus gravity along the nose, without the induced drag.
  The tests bound how far off these approximations are, but each one is an approximation.
- **Only tested to 500% plane speed** (the slider's most), and only on the disc's biplane file. A mod plane
  with odd cm terms would need its own check (a `cm_de` of 0 turns the limiter off).
- **`Sim` grew 8 bytes (11352 → 11360, tests/hash.rs pin moved); the leaf count stays 909.** The snapshot wire format grew two floats per Flight. Older snapshots would not read, though nothing
  persists them across builds.
- **The menu row was not looked at on screen.** It was not rendered or shot; it is covered only by the build
  and the existing menu tests.

## Next

Nothing. If someone wants the ^2 ease from the brief anyway, it is one line in `Aero::g_limit`.
