# 2 — world step on mxu (PART)

Task MD123. 2026-11-10.

## What I did

- Checked the dependencies again. MD122 is on main now. But `quad::Load` still takes the half box and the
  wheel mounts and sweeps as inputs, and MD129 (filed by MD122: those from the quad's model) is open and
  says "before MD123". So a quad made from its files alone, which the task's gate test asks for, still
  can't be built. MD123 is blocked `--after MD129,MD123a`.
- I didn't wait on that for the parts that don't need it. In `crates/game-sim/src/quad.rs`:
  - `quad::Ride` is the type a shell steps once a tick. It holds `Params`, `Quad`, the rigid body,
    `Bike` and a `rest` (a `Vec<Call>` by default). It has:
    - `controls`, which lays the gas, brake and stick where the pad reader stores them;
    - `frame`, which is `quad::frame` with any ground query and `Sweep`;
    - `frame_on` and `step`, the same on an MX Unleashed `world::World`;
    - `wreck`, the reason of the first `Call::Wreck` since `rest` was last cleared.
  - `quad::world_ground` is the ground query on an MXU world: `World::ground`, plus `World::wheel_surface`
    into the wheel's record when the world writes one. This is the query `ride::Ride`'s wheels ask.
  - `quad::OnWorld` is the `Sweep` on an MXU world. It stands the trigger probe's four segments at the
    node's frame and sweeps them as one bundle, through `World::solid` and then `World::groups`, into
    one record. Each segment that hit becomes a contact at its own start, so `wheel_contacts` hands it
    to its wheel. `gather` is answered and does nothing, because the world sweep reads everything.
  - `quad::world_bounds` is the in-world test's box: the world's `area` from the origin, or none.
  - `quad::WORLD` is the quad's physics world (1 iteration, 115/s).
- Tests:
  - `tests/atv_quad/frame.rs`'s chain (MD37b: the spawned quad's 240-frame ride on Desert Oasis, bit-exact
    against SLUS-21104's world step) now steps through `quad::Ride`, with `controls` and `frame`, in place
    of a hand-built `Steps`.
  - New `tests/atv_quad/mxu.rs` rides the image-seeded quad on free09 headless through `Ride::step`.
- Split the frame tail, the record, the wreck and the put-back off as MD123a (filed). None of that needs
  MD129.

## Took from opentennis

Nothing. opentennis has no vehicle physics.

## What the original does

Nothing new was read. The original has no quad on MX Unleashed's tracks, and the world step's code is
MD75's port, unchanged.

## How it was proved

- **The Oasis chain through the new type.** `frame::the_spawned_quads_ride_is_the_originals_frame_after_frame`
  and `..._run_into_the_palm_...` pass through `quad::Ride`. I checked they really ran with a temporary
  panic: 240 frames, 3840 ground queries, every word equal to the original's every frame.
- **free09 headless.** `mxu::the_spawned_quad_rides_free09_headless` puts the quad down on free09 at
  (1872, 1900) and runs three phases: 60 frames standing, 180 on the gas, 120 on the gas with the stick hard
  right. I checked this one with a temporary panic too:
  - 493 ft gone, top 106 ft/s;
  - all four wheels down on 266 of 360 frames;
  - lowest 0.48 ft over the ground;
  - turned 2.10 rad on the stick;
  - no wreck;
  - only `GravityRecord` and `PartMove` handed on.

  The asserts are top > 20 ft/s, > 50 ft gone, > half the frames all four down, never 1 ft into the ground,
  > 0.3 rad turned, and no call outside the known dropped list.
- MD37b's C28 replays (tests/atv_quad/replay.rs) are not touched by this change. The gate runs them.

## Not verified / not 1:1

- **Not a quad from its files.** The free09 ride uses MD37a's spawned quad, read out of
  `atv-r-spawn.ee.bin` and moved by tests/atv_quad/oasis.rs's `place`. A files-only quad waits on MD129,
  for the half box and the mounts and sweeps from the model. It also needs the init's spawn half (scene
  nodes, collidables, probes, pose), which MD122 left out. That is MD123's own remainder.
- **No capture can prove the free09 ride.** The original has no quad on MXU tracks. The test proves only
  that it rides: it gets going, turns, and stays on the ground.
- **The surface numbers aren't remapped.** `world_ground` hands the quad MX Unleashed's surface index. The
  quad's tire tables are indexed by SLUS-21104's surfaces, and the level setup's remap (`remap_surfaces`)
  isn't run for an MXU level, so I haven't checked whether the numbers mean the same surfaces in both
  games.
- **`OnWorld`'s contacts aren't the original's.** It sweeps MX Unleashed's props (`World::solid`, the bike's
  sweep) and not SLUS-21104's nearby search over its spatial database:
  - the bundle's sphere is the frame's position, with the farthest segment end as its radius, not the
    probe tree's root box;
  - a contact's node is the owner number of the prop it hit;
  - no contact has an owner or a vehicle, so nothing is notified.
- **Calls dropped on MXU (`rest`):**
  - `GravityRecord`: no record is held, so the contacts a strike files never reach the body. That is MD123a.
  - `SpatialMove` and `PartMove`: an MXU world has no spatial database.
  - `BodyReset` and `Respawn` (origin off the world box), `PreloadCue`, `StartStunt`/`EndStunt` and `Wreck`:
    listed in `rest` and acted on by nobody yet.
  - The free09 ride itself handed on only `GravityRecord` and `PartMove`.
- **Not run in `quad::Ride` at all yet:** `quad::update` (the tail's hazard surface, rumble, stunts,
  countdown and posts), the collision record's clear and pass, the wreck body on WreckATVm.col
  (`build_wreck_body`, `swap_rays`), `put_back`, and a jump. All of that is MD123a.
- **`Ride::frame` clones `Bike` once a frame** to hand it to `Steps` by value. The cost wasn't measured.
- **The bounds' floor is kept as the image left it** (Desert Oasis's). Only the box is the MXU world's.

## Next

- MD123a: the tail, the record, the wreck and the put-back in `quad::Ride` on free09.
- Then MD123 itself, after MD129: build `Quad`, `Bike` and the body from `Load` with no image (the init's
  spawn half), and ride that quad on free09 through `quad::Ride`.
