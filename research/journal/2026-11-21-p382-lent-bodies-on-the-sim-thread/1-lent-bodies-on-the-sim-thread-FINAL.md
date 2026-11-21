# 1 — lent bodies on the sim thread (FINAL)

Task P382. 2026-11-21.

## What I did

- The rider body (`game_render::rider::Body`) can be copied now, cheaply: the animation set's bound clips
  sit behind one shared list (`anim::Clips`, an `Arc`, copied on write when a clip is bound) and its
  archives behind an `Arc`. Every pose, clock and table is the copy's own.
- A clip channel's pair hint (`skin::RotChannel::cursor`) was a `Cell`; shared clips cross threads, so it
  is an `AtomicUsize` (relaxed) now. Same reads and writes, same order on one thread.
- `main.rs`: the B43 rule that kept every tick with a lent body on the loop is gone. A tick sent to the
  sim thread lends each seat's and race AI rider's body to its rider's step (`lend_body_kept`) and goes
  with the world; the rig keeps a copy of the body as the last tick left it, which the pictures drawn
  meanwhile draw, as they draw the copy of the seats. When the tick lands, the body itself goes back
  over the copy (`take_body`), and the tick's posed flags are set as on the loop. A tick still in
  flight when the ride ends gives its bodies back too.
- `tests/tiled_world.rs`: the split-screen same-ride test runs twice, riding ragdoll off (as before)
  and on (new, P382).
- Took nothing from ../opentennis: it has no sim thread with lent bodies.

## What the original does

Nothing to port: the sim thread is the port's own (N23a). The ride must be the same whichever thread runs it.

## How it was proved

- `a_worlds_ticks_with_lent_bodies_on_the_sim_thread_are_the_same_ride` (tiled_world.rs): md8_world,
  two seats, riding ragdoll on, 360 ticks, `--sim-thread on` vs `off`: every seat's state hash after
  every tick the same, 358 ticks on the sim thread. The same ride with `OPENMX_RIDING_RAGDOLL=0` gives
  other per-tick hashes (tick 200: `0x1653…` vs `0x9519…`), so the riding ragdoll was live in it.
- By hand: a one-player sx01 race with three AI riders (`--race --laps 3 --racers 4`), 300 ticks from
  one recorded tape, `--sim-thread off` vs `on`: all 300 tick hashes the same, 298 on the sim thread.
  (Without a tape two `off` runs already part at tick 1, so a race needs the tape to compare.)

## Not verified / not 1:1

- The race AI riders' case has no test in the gate: proved once by hand (above). Reason: a race needs a
  recorded tape and three debug-build rides; the seats' case, which runs the same lend/take code, is in
  the gate.
- The clip pair hint is now shared between a body and the copy a picture draws. A picture that samples
  clips (a plane pilot's, a car driver's pose) can move the hint while the tick on the other thread
  samples the same channel, where on the loop the two came one after the other. The hint only says where
  the key search starts; I did not prove the answer is bit-identical for every start (at a time exactly
  on a key two starts could pick neighbouring pairs). The test rides above did not fly a plane or drive
  a car while threaded.
- A picture that writes to the body (pilot and driver poses bind clips, `set.load`) now writes the copy,
  which is dropped when the tick lands; on the loop it wrote the body itself. Those clips are bound again
  by the next picture on the body itself. Not ridden threaded with a plane or car up.
- The copy's cost per threaded tick (poses, the claims list, the skeleton) was not timed.

## Next

None.
