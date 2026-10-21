# 1 — bike reset ghost fade (FINAL)

Task P57. 2026-10-21.

## What I did

- Followed the bike's hand reset through the decompilation to the routine that turns the model
  see-through and the one that fades it back (addresses in `research/oracle.txt`'s P57 block).
- Watched it live on copy 3 with `capture/ghost_watch.py` (freeride state 8, which starts mid-wreck,
  so one run shows the wreck's put-back and then an L1+R1 put-back 27 ticks into its ghost). The
  per-tick log is `capture/ghost_watch-state8.txt`.
- Ported the clock and the alpha as `vehicle::Ghost` (`crates/openmx/src/vehicle.rs`), in `ps2_float`
  arithmetic in the original's order (the `adda`/`madd` pair and the two clamps). `Garage::step` ticks
  it before the bike's frame, the way the original's update does, and starts it when the frame says
  `respawned`. The plane's arm does not touch it.
- `game_render::gpu::Actor` has a new `alpha` field. An actor below 1 is drawn after everything solid
  and translucent in its view, through a second pipeline for each material whose blend is the
  constant colour: `src·a + dst·(1−a)`, or `src·a + dst` for an additive material. `pass.set_blend_constant`
  carries `a` for each ghost batch, so the shader is unchanged. Actors at 0 or less are not drawn.
  `main.rs` gives the bike, its parts and the rider `seat.garage.ghost.shown()`, and the plane 1.

## What the original does

- Every put-back of the bike goes through the respawn routine with a non-zero reset kind: the
  wreck's own respawn and L1+R1 are both kind 1. Before the bike is placed, the model-setup routine
  (`./kit fn 0x00250580`) runs with that kind. It sets the ghost clock to its length (3.0, written by
  the rider's constructor rather than read from a file) and the state word to 2. It sets the
  see-through flag on the bike's model and every part, with alpha 0.5. It also adds the vehicle
  class to both bodies' ignore lists, so other machines pass through a ghost.
- The ghost step (`./kit fn 0x00250950`) is the first call in the bike's update. While the clock is
  above 0 it runs an overlap query on the body. Only if nothing overlaps does it take the tick's dt
  off the clock. While the clock stays above 0 it writes
  `alpha = clamp(0.1 + (clock − len)/(0 − len) · 0.9, 0.1, 1)`; once the clock is spent it clears the
  flag and the ignore mask and sets the state back to 1.
- Live, on the put-back's own tick: clock 3.0, alpha 0.5, every part 0.5. Then 0.109999,
  0.119998, 0.129997 … 0.989910, 0.999909 over 90 ticks at dt 0.03333. On the 91st tick the clock
  is −0.033028 and the model is solid. A chord inside a running ghost starts it over at 3.0.
- The plane has none (S57). The machine swap (`machine_swap` in the oracle; R8) hides the bike it
  leaves and puts nothing back. Taking the bike back is a hand-over, not a respawn, so nothing starts
  the ghost there.

## How it was proved

- A bit-exact fixture against the live capture: `crates/openmx/tests/ride.rs`'s
  `the_put_back_ghost_fades_in_as_the_originals` steps `Ghost` at dt 0.03333 and compares the clock's
  and the alpha's bit patterns with the game's on ticks 1–5, 89 and 90. It checks the clock's bits on
  tick 91 and the switch back to solid, and that a second put-back mid-ghost starts it over.
- Through the shell's garage and pad: `l1_and_r1_put_the_bike_back_as_a_ghost`. It rides two seconds
  of the minute and presses L1+R1. That frame is at half, and the ghost lasts 180 frames, ±1 (3 s at the
  test's 60 Hz dt). The plane's chord starts no ghost and does not tick the bike's.
- The renderer: `game-render`'s `a_ghost_actor_is_drawn_at_its_alpha` draws a board as an actor on the
  real GPU. At alpha 0.5 all 3757 of its pixels land halfway (±2) between the fog and the solid draw.
  At alpha 0 nothing is drawn.
- By eye: the shell headless with its default track and bike, on a generated tape (100 ticks of gas, L1+R1
  on tick 140). `capture/shell-ticks-175-200-240.jpg` shows ticks 175, 200 and 240: the ground shows
  through the bike and the rider at 175, and they are nearly solid by 240.

## Not verified / not 1:1

- **The overlap freeze and the vehicle-class ignore are not ported.** The port has no machine-vs-machine
  collision (the riders do not touch each other), so there is nothing to ignore. That also makes the
  clock's "only while nothing overlaps" always true. When rider-vs-rider contact lands, the ghost has
  to ignore it and stop counting while overlapping (filed as a follow-up).
- **Online seats have no ghost.** Online riders are stepped outside `Garage` (`online.rs`), and the
  ghost is not in the rollback state, so a networked put-back cuts solid. Filed as a follow-up.
- **The software rasteriser draws ghosts solid.** It is the fallback renderer and has no blend for a
  whole model. Only the GPU path is see-through.
- **How the ghost is drawn is by eye, not the original's draw.** The flag's other write (model +0x148 =
  1.6) was not decoded. The port's ghost writes depth and draws each part in batch order, so a ghost
  hides its own far side. Whether the original's ghost does too was not checked against a screenshot
  side by side. A translucent material in a ghost uses the constant alpha alone, not its texture alpha
  times it.
- **The shadow:** the brief says the original's stays opaque. The port draws no bike or rider shadow,
  so there is nothing to keep opaque and nothing was checked.
- **The rider's ragdoll/body alpha** in the original was not read separately. The capture read the bike
  model and its 7 parts, which all carried the model's alpha. The port gives the rider's body the same
  alpha because S57's video shows the rider see-through too.
- **One place only:** captured on freeride state 8 (free09). Other tracks and the race modes use the
  same routine, but they were not watched.
- **The M key and P41's menu switch get no ghost.** The original's swap puts nothing back. Asked in
  `plan/HUMAN.md` rather than guessed.

## Next

None for this task. Follow-ups are filed: P59, the ghost for online seats, and S63, the ghost's collision once
riders touch each other.
