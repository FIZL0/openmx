# 1 — torsion spring (FINAL)

Task S196. 2026-11-02.

## What I did

- `prop_world::Spring` (crates/game-sim/src/prop_world.rs): the body it turns, its switch, and its
  stiffness and damping. `Spring::new` is the reader: stiffness `xyz` = `Spring.xyz × inertia`, `w` =
  `Spring.w`; damping per lane `2 × zeta × sqrt(inertia × stiffness)`, `w` 1 (the constructor's).
  `Spring::force` is the force listener: nothing unless on and the body awake; the body's turn since
  its start turn, `unturn(start, rel(quat, start))`, through `ragdoll::axis_angle`; the spin about
  the axis (`rates · axis`, FPU multiply-accumulate order); `−(stiffness × angle + damping × spin) ×
  axis` added to the body's moment.
- `Collection::springs`, made in `Collection::new` for each `BodyTorsionSpring` whose part is there
  (else `Unmade::NoPart`). The force list is the springs first, then the objects' force listeners:
  the original files the spring on the world's force list at load, the objects' listeners only on
  enable. `Unmade::TorsionSpring` is gone.
- The bones of a loaded collection carry the body's inertia now (`props::Prop::inertia`, from
  `props::bodies`); it was zero, and only the spring reads it.
- The springs are the track's: `snapshot.rs` skips them on the wire and the decode keeps the
  collection's own (the S131 way of keeping track-made data off the wire). No wire size moves.
- `Collection::new` set the collection's `ResetTime` to −1 whatever the file said; it now takes
  `Pco::reset_time` (`[PhysicsParts] ResetTime`, −1 when absent). The countdown itself (S130's
  settle) was already there, never fed.
- `ragdoll::rel` is `pub(crate)` for the spring.

## What the original does

The constraint reader makes a 0x40-byte spring whose vtable's force slot is the pull; it is filed
on the collection world's force list straight away, so in a woken banner it runs before the
object's force listener (gravity). The pull is in the body's frame, onto the moment. Everything is
in research/oracle.txt's S196 block.

## How it was proved

States: `context/states/s196-sx01.ee.bin` and `s196-free11.ee.bin`, from S133's `boot.py` (sx01 race,
free11 free ride) in PCSX2 copy 2, saved to slots 21/22 and dumped with `./kit state ram`. Only in
s2's context (git-ignored).

crates/game-sim/tests/prop_spring.rs:

- `sx01s_banners_load_the_springs_the_original_read`: `PropSet::load` off the disc alone makes sx01's
  27 collections; the nine banners (the only collections with a force-list item asleep) each hold one
  spring, and its stiffness, damping, switch and body, the bone's inertia (body +0x110) and the pin's
  limits are the state's bit for bit; the state's force-list item is a torsion spring whose force
  slot is the pull, and the body has no listeners for the moment to be told to.
- `a_swung_banner_springs_back_sleeps_and_is_put_back`: banner_09 woken in both (activate, enable)
  and spun 8 rad/s about its pin's free axis; the original's collection update and both node
  listeners run in the interpreter beside `Collection::tick`, every frame the body (all fields), the
  node's rows, both countdowns, fresh, active and the listeners left compared bit for bit. It swings
  to 69.9° (the ±70° limit), the spring brings it back, it sleeps at frame 99, its own five-second
  countdown puts it back at 250 and the run ends with both sides' listeners gone. The same swing with
  the springs taken off parts from the game's turn at frame 1.
- `a_bowling_pin_waits_out_its_collections_reset_time`: every free11 collection's ResetTime is the
  state's (this failed before the fix: −1 against 5 on the pins and spare tyres). Then free11's
  bowling-pin collection (ten bodies, one part as a group) is rebuilt off the state, one pin is woken
  with a 0.3/−0.2 rad/s spin and its gravity zeroed in both; it sleeps on frame 1 (under the 0.5
  thresholds), the collection counts down for 151 frames, resets every pin and deactivates — every
  body and node of the ten compared bit for bit each frame.

`./kit check -- -p game-sim`: 939 passed (the three new ones and S130/S131's prop_world, prop_set,
snapshot among them). `cargo check --workspace --tests` builds. `./kit audit` flags one leak in
plan/TODO.md:34, which came with main's P256 plan entry, not this task.

## Not verified / not 1:1

- **PinToBody**: no `.pco` on the disc has one, so there is nothing to run it against; the port's
  `pin_to_body` path (S130) stays unproved.
- The banner's swing is a spin poked into the rates, not a captured strike; the bowling pin's
  likewise, and its gravity is zeroed on both sides because no collide pass runs in the test (with
  gravity it would fall through the ground for ever). Through the real ride, a strike and the
  ground contacts are S131/S197's.
- Only sx01's nine banners are checked against a state; the other 23 sx tracks load the same
  `banner.pco` (their loads are not compared).
- The bowling pin sleeps on its first frame, so the run proves the collection's countdown, reset and
  deactivation, not a pin's tumble.
- The two states live only in s2's context/.

## Next

None.
