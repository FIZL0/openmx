# 1 — wheel owner (FINAL)

Task S168. 2026-10-28.

## What I did

The brief asked me to put the other riders' bike collidables in the ride's wheel search, set the
wheel's owner word (`+0x1f0`, `wh.mv.fresh.scalars[2]`), and read `under()` through
`contact::other_at`. **I did not wire the other riders' bikes in.** The original's wheel probe never
accepts another bike, so adding them would make the port differ from the original. What I did instead:

- Two oracle tests in `crates/game-sim/tests/bike_col.rs`:
  - `the_wheel_probe_never_stands_on_another_bike`;
  - `a_physics_trucks_mesh_is_what_a_wheel_could_own`.
- `ride.rs`: two doc comments changed, no code. One is on `wheel_contact_velocity`, the other is the
  comment on the zero owner in `contacts`. They now say why the owner stays 0, which is the
  original's behaviour, not a gap in the port.
- `research/oracle.txt`: an S168 comment block under S145's.
- Filed **S206**: once S169 simulates a woken HasPhysics truck, put the truck's physics mesh in the
  wheel search with its owner, and read `under()` through `other_at`'s body side.
- Nothing taken from opentennis: it has no counterpart for this.

## What the original does

- **The wheel ray probe.** The wheel probe is the bike's second ray probe (`+0x7c8`, two segments,
  S114). It searches the bike's nearby search (`+0x7bc`) through collide_against_search, which asks
  `collision_pair_filter` (`./kit fn 0x00119d70`) for each candidate.
- **Its category list.** S86 read the probe's `+0xe8` list as `[0x100, bike +0x390]`, mode 1, which
  means "never collides with". `+0x390` is `0x2000` on every bike: both split-screen players (S86), and
  free09's AI bikes (S134's records have actors `0x2008`, and this task read `+0x390` directly).
- **Why every bike is refused.** Every bike's frame (`0x9a048`) and wreck part (`0x9a058`) carry the
  `0x2000` bit, so the filter refuses them all. So did every other collidable with an owner in these
  states: the biplane and the second machine (`0x42058`), and the `0x52058` object in recording2.
  Those all have dynamics bodies (`+0xf4`).
- **The one owned collidable the probe is not barred from:** a HasPhysics truck's physics mesh (car
  `+0x698` → `+0x128`).
  - Its category is `0x18`. Its `+0x80` is a collision object with no chassis and a body.
  - On free06 (s89-along), the filter accepts all four riding bikes' wheel probes against it when asked
    without the nearby bit, and refuses when asked with it. The sleeping truck's mesh has the nearby
    bit clear.
  - So a wheel's owner branch could only fire on a truck that has been woken (struck). The port does
    not simulate that (S127, S169).

## How it was proved

All of this is function-oracle work: the original's `collision_pair_filter`, run in the EE
interpreter on real RAM images.

- `the_wheel_probe_never_stands_on_another_bike`:
  - **S93's three split-screen images** (`splitscreen`, `s93-short-5997`, `s93-long-3402`), each
    player's wheel probe:
    - checked that it is a ray probe with two segments;
    - asked about the other player's frame and wreck part, both with and without the nearby bit:
      refused all 6 times, with both answers the same;
    - control: the body rays against the same frame are accepted;
    - every owned candidate in the bike's nearby search (3 for each bike) is refused.
  - **recording2-ram.bin**, S134's three AI bikes:
    - `+0x390` is `0x2000` on each, and their frames are where S134 read them;
    - each bike's wheel probe refuses the other two frames, and all 4 owned candidates in its search.
  - **The port's side:** `world_search::accepts` over `bike_col::collidables` for two seats. The wheel
    probe refuses the other seat's frame, and the body rays accept it. S86 already proved the
    collidables match RAM, and world_search.rs already proved `accepts` against the original.
- `a_physics_trucks_mesh_is_what_a_wheel_could_own`, on s89-along:
  - finds both HasPhysics trucks by the class's vtable;
  - checks each physics mesh's category is `0x18` and that it has an owner;
  - checks the filter answers `[true, false]` (without / with the nearby bit) for each of the 4 riding
    bikes found.
- I checked both tests really ran rather than skipped: I added a temporary panic and read the
  `--nocapture` lines in check.log (9 bikes compared in the first test, 2 trucks × 4 bikes in the
  second), then took the panic out again.

## Not verified / not 1:1

- **The owner branch is still not reached by any ride in the port.** That is the original's behaviour
  for bikes, AI bikes, wreck parts and the machines, as proved above. A woken HasPhysics truck's
  physics mesh is the one exception, and its wiring waits on S169 (filed as S206).
- **Only the states I read.**
  - I read S93's split-screen free09 images, recording2's free09 session and s89-along's free06. I did
    not scan other tracks for other owned collidables outside category `0x2000`.
  - The scan only covered what was in each bike's nearby search at that moment, plus the trucks found
    by their vtable. A dynamics object in another class, on another track, could still be one the
    wheel accepts.
- **Ragdoll bone spheres were not checked.** A downed rider's ragdoll bodies were not in any of these
  searches, so I do not know if a wheel probe accepts them.
- **Whether a woken truck's mesh is searched at all is not known.** The wheel's search asks the
  filter with the nearby bit, and the sleeping truck's mesh has that bit clear. I did not wake a truck
  live to see whether its bit gets set, or whether the mesh is in a bike's nearby search then. No live
  capture, because there was nothing in the port to compare it against.
- **The nearby argument of the wheel's own search.** I took it from the port's `search_nearby` (world_search.rs's
  proof of the nearby search) and from oracle.txt's S129 notes. I did not trace the wheel's
  collide_against_search call itself for this task.

## Next

S206 after S169.
