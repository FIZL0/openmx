# 1 — one generator for the rest (FINAL)

Task S387. 2026-11-18.

## What I did

- **Each rider's frame borrows the race's generator** (`Sim::lend_rng` / `Sim::rng_back`, lib.rs). Before
  player i's bike frame the race's word goes into the rider's pool (`Rider::idles`, seeded). After the
  frame it goes back to the race, where the frame's draws left it. The rider's own pool is put back as
  it was. This covers every draw the rider's update makes off the pool: the edge throw and its entry
  spin, the wreck clank and voice, and the ragdoll's wreck entry. All four Sim step paths do it:
  `step` (both branches), `step_all`, and the online `step_seat` (rider_contact.rs).
- **The riding ragdoll draws on that same word, after the update's draws.** A crate-private flag,
  `Rider::lent_rng`, is set only for a lent frame. With it set, `riding_tick` swaps the riding body's
  own generator with the pool's around its tick. So the ragdoll's draws come right after the update's,
  on one stream. Its own generator is untouched. The flag is not sent (snapshot.rs skips it) and not
  hashed.
- **A plane in a race draws on the race's generator.** `Flight::rng` is swapped with the race's for the
  plane's frame in `Sim::step`, then swapped back.
- **The bodies' look-around, the engine notes and the dust now pick the race's generator before the AI
  lane's.** Before, they took the lane's generator whenever a lane existed. In a race with AI riders
  that drew on the lane's own, which S333 says must stay still while the race lasts.
- opentennis: nothing to take (it has no shared race generator).

Order in a tick is the one S333 set: the brain thinks after its bike's frame, then dust, and then, after
every bike, the bodies, the notes and the race manager. The rider's own and ragdoll draws now sit
inside its bike frame, before its brain.

## What the original does

The C library's one `rand()` serves every subsystem (see S333's journal). The rider's wreck draws and
the riding body's draws are calls on that same generator, made inside the rider's update and the body
actor's update. In the port that body actor update runs at the end of the rider's frame (S90/S110).

## How it was proved

- `tests/off_the_map.rs::a_races_rider_is_thrown_on_the_races_generator`. One rider, set past the
  margin with a wreck body, ridden 200 ticks in two Sims: one with a race whose tick draws nothing, and
  one with no race and the rider's pool put on the race's start word. Each tick the race's generator
  equals the lone rider's pool, the rides are the same, and the race rider's own pool never moves. The
  throw does draw: the final word differs from the start. With `lend_rng` forced to lend nothing, the
  test fails.
- race, rider_anim, hash, platforms, snapshot and off_the_map suites pass.

## Not verified / not 1:1

- **Not checked against a live capture.** No capture of the original shows a race rider wrecking or the
  riding ragdoll drawing while the per-frame rand word is recorded. S291's sx01 capture has no wreck.
  The order inside a frame (update, then riding body, then brain) follows the port's existing actor
  order (S90, S323). It was not traced in the original's frame.
- **The riding ragdoll's part is not tested.** Its tick needs a ragdoll kit from the disc and the shell's
  holds. No test sets up both inside a race. The swap is two lines in `riding_tick`, but no test runs it.
- **The bodies, notes and dust now prefer the race over the lane.** No test has a race with AI riders
  plus notes or dust, so the changed pick is not run by any test.
- **The plane in a race is not tested.** The original's races have no plane. The Sim's plane branch also
  never runs the race tick, so no race draws happen beside the plane's.
- **Quad, atv_car and vehicle_anim are left as they were.** Reasons:
  - The quad and car lanes are stepped by openmx's online rides (net/rides.rs), not by a Sim step that
    ticks a race.
  - A quad or car in an MX Unleashed race is a mod addition, not part of the original.
  - `atv_car::Vehicle` is not held by the Sim at all.
  - `vehicle_anim::Shared` (the start-pose chooser) is only used by tests. The Sim never draws on it.
- **Wreck bikes downed beside a plane** (`wreck_plane::step_downed`) still draw on their riders' own
  pools in a race. That path runs only in the plane branch, which has no race tick.

## Next

Nothing for this task.
