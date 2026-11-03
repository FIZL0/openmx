# 1 — kiowa blades (FINAL)

Task S220a1a. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs`, new section "the thrown blades":
  - `BLADE_MODEL` / `BLADE_RAYS` / `BLADE_SHAPE`: the three machine.res names.
  - `Blades { set: Collection, started: [Started; 4], rays }`.
    - `Blades::new(phy, psg, rays, edge)`: each body is built from `wreck_blade`'s part:
      - inertia = the part's inertia of `NodeBox::from_model(bladebent.psg)`'s box;
      - mass and CofG from the part, via `WreckBike::new`;
      - the object is the props' generic physics object, with its collision off the m.col rays;
      - the put-back place is the origin, look +z, up +y, through `Collection::place`.
    - `start` / `stop(hide)`: wreck_part_start / stop. They cover:
      - the active bits (`Started::out`);
      - node shown and placed;
      - the probes in and out of the broad phase (`Started::probes`, when CollisionEnabled);
      - enable, or disable plus the listener taken off and the collision unfiled
        (`Collection::take_off`).
    - `throw(k, at, facing, up, velocity, rates)`: start, then the place swapped in only for the
      reset (the original hands physics_object_place stack copies), then velocity and rates.
    - `search` / `step` / `post`: the collide pass, the world step up to the posts, and then the
      posts plus the object listeners. They are split because the heli's post (crash test, act)
      comes first in the world's post list. The frame order is the heli tick plus `Blades::step`,
      then `Wreckage::act`, then `Blades::post`.
  - `Wreckage::blades` is now a `Blades` (the old `Blade` is gone). `throw_blades` places and starts
    them at the node point less the blade's CofG. `unwreck_mx` stops all four with the hide.
- `crates/game-sim/src/prop_world.rs`:
  - New `Collection::step_to_posts`: the world step without its posts. With no bodies awake, the
    preforce still runs, as the shared world always holds the heli.
  - New `Collection::step_posts` and `Collection::take_off`.
  - `disable` now drops its entries by swap with the last (`unlist`). The original's lists do this.
    Seen on the post list: [1,0,2,3] less 1 is [3,0,2]. The post loop re-reads the count, so it
    matters when a post disables itself: an off-edge blade skips the one moved in that frame. The
    old shift skipped a different entry, and a thrown blade's node went unwritten. The whole
    game-sim suite still passes with it, apart from the pre-existing `the_humans_tricks` red.
- `research/oracle.txt`: an S220a1a block covering the wrappers, files, offsets, start, stop and the
  list removal, plus physics_object_listen.

## How it was checked

All in `crates/game-sim/tests/kiowa.rs`, on S148's live helicopter:
- `the_blades_are_built_as_the_originals`: for each of the four wrappers:
  - the body, every word;
  - mass, inertia and the start turn;
  - gravity, CollisionEnabled, DetectCollisionsWhenDisabled, ResetOffEdgeOfWorld and the listener
    switch;
  - ResetTime and the thresholds;
  - place, look, up and the countdown;
  - the probe's rays (sorted);
  - the shared world node and world, and its iterations and rate.
- `the_wreck_and_the_put_back_are_the_originals`: wreck_part_start and wreck_part_stop are no
  longer stubbed. Each case also starts or stops each blade at random in both, with a random hide.
  `same_blades` compares:
  - the body;
  - the active bits, node shown and placed, and the three probes in the broad phase;
  - fresh, enabled, place, look, up and the countdown;
  - membership of the world's body, force, preforce and post lists;
  - the node listener and the filed collision.

  10,000 cases, bit-exact: 9,989 starts, 4,990 and 5,123 stops (hide 0 and 1), and 2,388 throws of
  blades already out.
- `thrown_blades_fly_as_the_originals`: 40 throws of 60 frames at 30 Hz. The world holds the heli's
  body and the blades. Each frame runs the original's dynamics_world_step and the node's object
  listeners against `Blades::step` / `post`. The throw's frame runs only the posts and listeners.
  After every frame it compares `same_blades` and each node's world rows (transform_rebuild is not
  stubbed here). Bit-exact. At the end, 128 blades were flying, 32 had been put back off the edge,
  and 125 had been thrown while out. Three cases in four are a calm heli mid-map.
- `./kit check -- -p game-sim --no-fail-fast`: everything passes but trick.rs `the_humans_tricks`,
  which is pre-existing (C6's capture).
- `cargo build -p openmx` builds. `./kit audit` shows only the 2 pre-existing plan/DONE.md leaks.

## Not verified / not 1:1

- No collision was proven here. The flight test runs no collide pass, so no blade touches the
  terrain or a prop. `Blades::search` rides on `Collection::search_among`, which the props' own tests
  prove.
- The f.col shape (+0x128) is not loaded, so other objects' rays cannot strike a blade.
- The categories are not compared.
- The node listener list's removal is assumed to shift down. Falling blades never come to rest, so
  it barely runs.
- Throw's rows still come from the caller, not from the rendered model tree.
- The body 0x8ef8a0 in the shared world is unknown.
- No drawing and no on-screen look beside a PCSX2 wreck. Openmx does not call any of this
  (P216a1).
