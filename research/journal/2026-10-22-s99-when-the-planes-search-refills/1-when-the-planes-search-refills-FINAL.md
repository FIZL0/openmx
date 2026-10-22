# 1 — when the plane's search refills (FINAL)

Task S99. 2026-10-22.

## What I did

- Found who calls plane_refresh_search (`./kit fn 0x15b210`), and on what clock. S96 had looked for a
  virtual call at vt +0x108 measured from owner+0x10. The plane's collision owner (plane+0x50) is a
  multiply-inherited class: its real vptr is at +0x38 (vtable 0x45b2ac), and five one-slot interface
  vptrs sit at +0x40..+0x50 with this-adjusting thunks at 0x15c260..0x15c2b0. The refresh is
  **vt +0x110** off +0x38, the slot body_wrapper_post (`./kit fn 0x15b070`) calls last. A scan of
  `lw v, 0x38(x) … lw t, 0x110(v) … jalr t` over the state's code (scratch vcall2.py) and the thunks
  found the three callers: body_wrapper_post; the asleep check 0x15b2a0 (via the thunk 0x15c2b0,
  which refreshes only while the body sleeps and +0x6c is set); and the wake 0x15b790.
- Traced it live in emulator copy 3. A PINE script loads biplane.p2m2's state into slot 8 and writes
  logging trampolines in the free RAM at 0x1fe0000. Each logs (tick 0x479d04, tag, ra, a0) and then
  runs the two instructions it displaced. It patches each entry with one atomic 64-bit `j stub; nop`
  and takes the patches out afterwards. Hooked: the refresh, collision_search_gather 0x218b80,
  body_wrapper_post, collision_record_collide 0x2708a0, dynamics_world_step 0x1f29a0,
  collide_against_search 0x11a9d0, biplane_unwreck 0x3536e0. Every traced tick (two runs, about
  ticks 10993–11004) has the same order:
  1. every collision record's contact search, the plane's first (its sweep, then its ray probe);
  2. dynamics_world_step for world 0x60b278;
  3. its post listeners: the plane's body_wrapper_post from biplane_measure (the call at 0x352980). That
     refreshes, which is **the one gather of the plane's search a tick**, around the ray probe's reach
     at the stepped pose;
  4. the two driving toys' body posts from 0x1f2c00, each refreshing its own search;
  5. other gathers, then the bike world 0x30f7ec.
- A second state with the evidence inside it: the same hooks, plus a copy step at the plane's refresh
  (a0 == plane+0x50). That step copies each moving-list slot's word and centre (+0x40), and each of
  the plane's found candidates' centres, into a ring at 0x1fc0000 (a count, then 8 records of 0x1000).
  Slot 9 was saved after six ticks with the hooks still in. Copied to
  `context/recordings/biplane_later.p2s` and `./kit state ram` → `context/states/biplane_later.eeMemory.bin`.
  Its last copy is of the state's own tick, so the save fell between this tick's gather and the next.
  The script is not committed; it is described here (scratch trace2.py).
- Test `the_plane_gathers_in_its_post_before_the_others_move` in tests/flight_candidates.rs. It builds
  the port's world plus the state's moving objects as stand-ins (as S96's 34 does), with each stand-in
  at the centre copied at the refresh. It builds the list at the search's centre and measures again at
  the ray probe's. Result: all 34 in order, **every distance bit for bit**. With the centres the state
  holds instead, exactly the candidates whose centre changed since the copy are apart: five, S96's
  five. The test also checks:
  - the copy's tick is the state's tick;
  - the copied slots are the list's;
  - the build centre is the first state's (the list was kept, not rebuilt).

  I raised the final count to 55 once to see it fail, which proves the test ran and did not skip.
- Port: `Flight::refresh` now runs first in the post listener and calls the new
  `flight_search::refresh`, which builds the world at the stepped pose and gathers. `Scene::new` no
  longer gathers every frame. It gathers only to seed a list that was never built, or one that names
  more collidables than the world has. The frame's search therefore uses the list the last post
  gathered. One consequence matches the original's code: after an unwreck/put-back, the next search
  still uses the list gathered before the put-back, because recovery does not gather (biplane_unwreck →
  vt +0x198 0x355b00 → 0x351df0 → 0x15a030 removes and re-adds the probes, 0x1ab1d0/0x119c60, with no
  refresh).
- research/oracle.txt: corrected plane_refresh_search to vt +0x110 off +0x38; added probe_object_moved
  and an S99 block with the callers, the clock and the recovery note.

## What the original does

Once a 30 Hz tick, after every collision record's contact search and after the plane's dynamics world
has stepped, the plane's post listener (biplane_measure) first runs the body's post. That sets the
drawn pose from the body, then calls the owner's vt +0x110. The refresh marks the three probes moved:
probe vt +0xa8 recomputes the ray probe's reach from its node's new world matrix, and the probe's box
is moved in its table. Then it gathers the plane's nearby search around the ray probe (kind 3). The
next tick's ray and sweep search those candidates. Objects that move later in the same tick (the toys
in their own posts, the bikes in their world) keep the distance measured before they moved. While the
body sleeps, the asleep check refreshes instead. The plane never sleeps in the traced ticks.

## How it was proved

- A save-state capture plus a live call trace: two hook runs in PCSX2 copy 3, 6 ticks each, with the
  same per-tick call order in every tick.
- `the_plane_gathers_in_its_post_before_the_others_move` over `biplane_later.eeMemory.bin`: the port's
  gather equals the state's 34 candidates, order and all 34 distance bits, at the centres copied at
  the original's refresh. The five off at the state's own centres are exactly the five that moved
  after it.
- `./kit check -- -p game-sim --test flight_candidates`: 4 tests pass. The full `./kit check` is red only on
  P104's menu test, which is red on HEAD too.

## Not verified / not 1:1

- **The first frame's seed.** `Scene::new` still gathers a list that was never built (a new plane,
  the first frame after `with_rays`). Where the original first fills the plane's search at spawn was
  not found. Its add-to-world path (0x119c60) does not gather.
- **Disabled/asleep body.** The port refreshes in every post. The original refreshes from
  body_wrapper_post only while the body is enabled, and from the asleep check only while it sleeps
  with +0x6c set. The port's plane body is never disabled, and +0x6c was not read.
- **The recovery tick, live.** That the frame after an unwreck searches the pre-put-back list is read
  from the code (recovery calls no refresh), not seen. The traced ticks had no recovery (the
  unwreck hook never fired), and the replay acceptance does not single out that tick.
- **`place()`/`reset()` outside the frame.** The port's spawn and swap move the plane without a
  refresh, as the recovery does. Whether the original's swap/spawn refreshes was not traced.
- **The state carries the patches.** biplane_later.p2s was saved with the hook jumps at seven function
  entries and the trampolines and copy ring in RAM. Do not ride it as a clean state; slot 9's earlier
  clean save was overwritten.
- **Only slot s3 has the second state.** Other slots skip the new test until `biplane_later.p2s` is
  copied in and `./kit state ram` is run on it (the skip message says so).
- **Ordering among the other bodies' posts** (the toys before the bike world) is as traced in these
  ticks; it was not tied to the code.
- `./kit check` (full) had one failure outside this task: `the_pauses_modifiers_take_when_it_closes_unless_held`
  (crates/openmx/tests/menu.rs:1844, 100 vs 90). It is menu logic over default settings that this
  diff does not touch. It fails the same way on HEAD (b63486a) in a scratch worktree; filed as P104.

## Next

Done unless the gate says otherwise.
