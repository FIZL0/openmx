# 1 — kiowa shell blades (FINAL)

Task P265. 2026-11-03.

## What I did

- `crates/openmx/src/vehicle.rs`:
  - `KiowaRide::blades`, built by `KiowaRide::with_blades(phy, psg, rays)`. It calls
    `kiowa::Blades::new` on `helicopter\wreckblade.phy`, `bladebent.psg` and `bladebentm.col`.
  - `KiowaRide::built` puts a copy into `Heli::wreckage.blades`. The edge is the terrain's: `world.area()`
    as `prop_world::Edge { 0,0 .. width,depth }`, the same as `prop_set`'s.
  - `KiowaRide::step`, while any blade is out (`Started::out`), runs four steps in order:
    1. `Blades::search` against `world.wooded()`'s field, props and trees.
    2. `Blades::step`, with the props' movers (`flight_search::movers`, as `PhysicsCar::tick` does).
       This runs before the heli's tick, because the crash test's `Wreckage::act` is inside the tick
       (`KiowaAround::crashed`).
    3. The tick.
    4. `Blades::post`. It runs if a blade was out before the tick or is out after it, so a blade thrown
       this frame is posted where it was thrown.

    With none out, none of it runs, because no blade is in the world's lists. The ragdoll `Tables` are
    built once (`blade_tables`, a `OnceLock`).
  - `KiowaRide::blades()` gives each shown blade's node rows. `KiowaRide::blades_report` is a
    `kiowa: blades: N shown, M moving, lo to hi ft over the ground` line for the ride's end.
  - `kiowa_throw`'s doc no longer says the shell builds no blades.
- `crates/openmx/src/kiowa_draw.rs`: `blades(heli)` turns those rows into `M4`s.
- `crates/openmx/src/main.rs`:
  - The lot reads the three blade files out of `machine.res` next to the heli's.
  - Both renderers hang `bladebent` (already loaded by `game_render::kiowa::Kiowa::load`) on each shown
    blade's node, beside the flown helicopter.
  - The headless ride's end prints the blades line after the `kiowa:` lines.
- `crates/openmx/tests/kiowa_switch.rs`:
  - `fast_landing` is now shared.
  - New `a_wreck_throws_the_blades`: the fast landing ridden to tick 530, still wrecked. It requires
    four blades shown, none more than 2 ft under the ground, and one over 10 ft up.
  - `a_landing_at_speed_wrecks` (600 ticks, put back) and `a_soft_landing_stands` now also require no
    blades line.
- `research/c14/wreck_shot.sh`: plays C17's strike tape on a kit copy and presses System → Screenshot at
  the given frames.

## What the original does

S220a1a's entry has it: the MX helicopter's four blade wrappers share its world, and their posts follow
the helicopter's in the world's post list. The frame is: the collide pass, the world step (heli and
blades), then the posts (heli first, whose crash test may throw the blades), then the node's listeners.

## How it was proved

- `./kit check -- -p openmx --test kiowa_switch`: 5 tests pass. On free09's fast landing the wreck lands
  on tick ~402.
  - On 403 the four blades are shown and moving, 8 to 14 ft up.
  - By 530 one lies 0.4 ft over the ground beside the road. One is 0.9 ft under the ground under its
    middle, in a dip, with its ends on the ground. One is still in the air.
  - The put-back (~540) hides them all.
  - Blade 2 rises to ~370 ft: it was thrown up at 90 ft/s and its vertical speed falls by about 32 ft/s
    per second, plain gravity.
- Looked right beside a screenshot:
  - The PCSX2 side is C17's `strike.p2m2` (s3's `context/recordings/c17`), played on kit copy 2 with
    `research/c14/wreck_shot.sh`. Its frame 1302, 4 ticks after the wreck, shows the four bent blades
    flying out from the rotor head as long, thin white bars.
  - The shell side is free09's fast landing, ticks 404 and 410 on Aero Cam 1, on the GPU and the
    software path. It shows the same thin white bars leaving the rotor head.
  - A posed close-up (`--shot-pose 4408.5,175,4085.8,58.6,-25.6,60`, tick 530) shows a whole 18 ft
    blade lying on the verge, on both renderers.
  - The shots are in the git-ignored `context/shots/p265/`.
- `cargo test -p openmx --no-run` builds every test target. `./kit audit` shows only the 2 leaks already
  in plan/DONE.md.

## Not verified / not 1:1

- **Not the same wreck on both sides.** The shell's look is its own fast landing on free09. PCSX2's is
  C17's rotor strike, at a different place and speed. The port cannot replay C17's tape through the
  shell. So the comparison is by eye: the model, its size and colour, and blades leaving the rotor head.
  It does not cover where each blade goes.
- **The collide pass is still unproven against the original.** S220a1a's flight test ran none. This task
  runs it, but where a blade lands is not compared with a capture. One blade came to rest with its
  middle 0.9 ft under the field (ends on the ground in a dip). That is plausible for the corner rays, but
  unproven.
- **Order inside the world step.** The blades step before the heli's tick, not interleaved with its
  substeps. Nothing couples them (the blades do not collide with the heli), so the order can only show
  through the shared props' movers.
- **Drawing.** The blades are drawn where the last tick left them, not slid between ticks as the heli is.
  Split screen and the second seat were not looked at. The blades are not in any netplay wire; the heli
  is not either.
- `bladebentf.col` (other objects' rays striking a blade) is still not loaded, as in S220a1a.

## Next

- Prove where a thrown blade lands against a PCSX2 capture of the blades' nodes. That needs a tape that
  wrecks the same way on both sides.
- Slide the blades between ticks like the heli, if a 60 Hz look shows them stepping.
