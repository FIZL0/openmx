# 1 — prop hit listener (FINAL)

Task S200. 2026-11-02.

## What I did

- `crates/game-sim/src/prop_world.rs`:
  - `Collision::categories` is now the ray probe's category list, kept as state. The init leaves it
    at every category (mode 1, empty). The disable of a body that has `DetectCollisionsWhenDisabled`
    and a ray probe sets it to mode 0 with the wake mask (0x40). The enable and the countdown's
    put-back set it back to every category.
  - `Collection::search` now searches through `Scene::probed` and no longer uses S198's `Waking` hit
    filter, which is removed. After each object's collide, it applies what the probe's listeners
    heard: the hit listener's wake (`enable`) first, then the collection's listener (`activate`).
- `crates/game-sim/src/flight_search.rs`:
  - New `Scene::probed`: the plane's world, the one the wreck bike also uses (`flown`, where the
    path cars are toys). It gives the ray probe the prop's category list and the prop's two listeners
    (`HIT_LISTENER`, `COLLECTION_LISTENER`). The list is used by both the gather's and the search's
    pair filter.
  - `heard_hit`: when the body is asleep and the struck object's category & `WAKE_MASK` is non-zero,
    it wakes the body. It then also sets the probe's list back to every category for the rest of
    that search, as the enable does. The collection's listener hears every hit.
  - `Heard` reports both. `Lent`'s drop puts the ray probe's list and listeners back, because the
    lend reuse does not restore them.
- `snapshot.rs`: the category list is on the wire (`wire!(world_search::Categories)`).
  `prop_set.rs`: the state walk includes it too.
- `tests/prop_wake.rs` (new): the proofs below.
- `research/oracle.txt`: an S200 block.
- Nothing taken from opentennis; it has nothing like this.

## What the original does

- **Hit listener** (`./kit fn 0x0015acf0`). It is the wrapper's listener on its ray probe, at wrapper
  +0x48; the thunk at `./kit dis 0x0015c290` subtracts 0x48. If wrapper +0x58 is set and the struck
  collidable's +0x84 & the global mask (0x40) is non-zero, it calls the wrapper's vt +0x124.
  - For a prop, vt +0x124 is the enable, 0x0015b790. The enable does nothing to an awake body.
  - +0x58 is set to 1 by the ctor (0x001590e0), and no part key changes it.
- **The probe's second listener** is the collection's (collection +0x30). Its thunk `./kit dis
  0x003b2b10` calls the collection's vt +0xa8, the activate, on every hit whatever the category.
  - The probe has just these two listeners, in this order (first_recording, BeachBall 0).
- **Category list** on the probe (+0xe8 mode, +0xec list, +0xf0 count):
  - The init sets mode 1 and empties the list (`0x0011b070`, `0x0011b1b0`).
  - The disable (0x0015b8c0), with +0x6c set and a probe, sets mode 0 and adds the global mask
    (`0x0011b080`, which adds it unless already there, into the first 0 slot or else at the end).
  - The enable, with +0x6c and a probe, sets mode 1 and empties the list again.
  - So does the object listener's put-back (S198's note).
- **In the state**, every asleep prop's probe has mode 1 and an empty list.
- **The egg's search list** once awake has these candidates:
  - first, the sculpture mesh (0x220, flags 0x1c, 1.44 ft away);
  - toys (0x48, flags 0x15, the trailers) at 14 and 38 ft;
  - AI bikes (0x9a048) and gates (0x200020).
  - All but the sculpture are beyond the rays' reach.
  - The port's prop world has only the scene meshes and plants (S239).

## How it was proved

Both tests run in the EE interpreter from first_recording (free09), with the port built from the disc.

- `every_props_probe_category_list_follows_the_originals`: covers every free09 object (at least 30
  ray probes). For each one, the port's list matches RAM (mode and words) at four points: as loaded,
  after the original's enable, after its disable, and after its object listener puts the object back
  once the countdown runs out. The put-back also unfiles the object and ends its listener.
- `a_wake_category_object_under_an_asleep_egg_wakes_it_as_the_originals`: the egg is enabled and
  disabled where it lies on LargeEgg's mesh, so it is asleep and filed with mode 0 [0x40]. Then 600
  frames run: clear, collide, the collection update and the node's listeners in RAM, beside
  `frame_among`.
  - Frames 1–30: the sculpture is 0x220 and the egg sleeps on, inactive.
  - At frame 31 the sculpture becomes a wake-category object: category 0x48 in RAM, and in the port a
    still toy (`Prop::moving` at rest).
  - Frame 31: the hit wakes it and the collection activates.
  - Over the run, counted at frame ends: it is awake 3 times, asleep again 3 times, and active for 132
    frames (the test prints this line).
  - Once it settles asleep on the wake-category object, each frame's search wakes it and its listener
    puts it back to sleep in the same frame. It creeps across the sculpture, and its countdown stays at
    ResetTime (60), so it is never put back. The test asserts it ends asleep, filed, with
    `countdown == reset_time`.
  - Every frame matches RAM bit for bit: position, velocity, turn, rates, the node, the record's four
    flags, the probe's category list, the countdown, fresh, the active bit, and the node's listeners
    in order.
- Mutations, each failing the test:
  - Ignoring the hit listener's wake fails at "frame 31: enabled".
  - Dropping the disable's setter fails the list test at "BeachBall object 0: disabled", and the egg
    test as well.
  - Searching with every category instead of the probe's list fails at "frame 1: touching…", because
    the asleep egg's rays meet the scenery.
- Also run, all passing: `./kit check -- -p game-sim` with prop_wake, prop_collision, prop_world,
  prop_knock, snapshot and hash (37 tests). The 23 suites that use flight_search, prop_set, wreck_bike
  or prop_world also pass (301 tests; traffic, ride and replay among them).

## Not verified / not 1:1

- **The wake-category object is made up.** free09 has no wake-category object within a prop's ray
  reach. The test relabels the sculpture's mesh as 0x48 in RAM, but it stays in the static tree with
  scenery flags and no mover. In the port it becomes a toy, which goes in the moving list and is a
  mover of kind 2 at rest. The results agree bit for bit on this run. The gather order of a real toy
  among several candidates, and a moving toy's mover velocity, are not exercised.
- **The props' world has no bikes and no trailer toy meshes.** Both are wake categories in the
  original (filed as S239). So in a ride, only the path cars (toys in `flown`, free06) can wake an
  asleep prop through its rays.
- **The enable's mid-collide side effects are applied after the object's collide.** These are vt
  +0x110, the probes' moved, and a re-gather that does nothing for a probe that has not moved. The
  probe's category list does change mid-search, as in the original. The rest of that collide, with
  the body now enabled, was compared only on this run.
- **The sweep probe's own category list is not ported.** The init sets it to mode 1 with two global
  words (`[0x468f50]`, `[0x468f90]`). It matters only for a sweep back, and no prop does one (S198).
  It is in S239.
- **Filing order.** The original's node runs collide in the order objects were filed. The port runs
  them in collection then object order (as in S198). The wake of one object, before the next object
  searches, follows the port's order.
- **Listeners on terrain hits.** Terrain hits are assumed to call no probe listeners, since they do
  not go through the nearby search. This was not checked separately.
- **Not run in PCSX2** with a real toy or bike under an asleep prop.

## Next

Done. The follow-up is S239: bikes and toy meshes in the props' world, and the sweep probe's list.
