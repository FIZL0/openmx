# 1 — biplane particles (FINAL)

Task P61. 2026-10-21.

## What I did

- `crates/openmx/src/particles.rs`:
  - `Emt` gains three fields:
    - `collides`: `GroundLevel` is present in the file.
    - `restitution`: default 0.55, the constructor's.
    - `wind`: the `[Properties]` `Wind` switch.
  - `[Properties]` switches are now read with the original's rule (`ini::switch`), not `== "T"`.
  - `Particle` gains `landed`.
  - `Particle::land` is the bounce or rest.
  - `PlaneFx::step_over(plane, dt, ground)` steps each particle, then tests the ground on odd ticks of the fx's own `tick` counter.
  - `PlaneFx::step` is the same with no ground; the tests use it.
  - `PlaneFx::wind` (zero) is added to a `Wind` emitter's start velocity, before the noise.
  - A CollisionModel entry with `HasCollision` turns `collides` on.
- `crates/game-render/src/biplane.rs`: `Emitter.has_collision` reads the `.dat`'s `HasCollision`.
- `crates/game-render/src/ini.rs`: `switch`, the original's reader for switch keys, with a unit test:
  - `T…`/`F…` by the first letter;
  - `ON`/`OFF` as whole words;
  - `Y…`/`N…` by the first letter;
  - anything else through `strtol` with base 0, on when non-zero;
  - case is ignored throughout.
- `crates/openmx/src/main.rs`: the fx steps with `step_over`. Its ground is the ride's own terrain query, `sim_world::World::ground` on `world.place`. That query reads the terrain only, and the fx writes nothing back.
- `crates/openmx/tests/biplane_particles.rs`, five new tests and an extended disc test:
  - The `.emt` keys and their defaults, and `HasCollision` on a CollisionModel entry but not a Generic one.
  - `land`, case by case:
    - deep under the ground: a bounce;
    - within 0.1 ft: rests on the ground;
    - exactly −0.1 ft: a bounce;
    - going up, above the ground, or not colliding: nothing happens;
    - a particle at rest still ages but no longer moves.
  - The odd-tick rule, counting how many times the ground is asked.
  - Chunks over flat ground lie on it, and over no ground they fall through.
  - The wind's addition.
  - The disc's files:
    - only Vehicle Chunks collides, with restitution 0.6 and gravity 2;
    - Vehicle Dust and Smoke take the wind;
    - the disc's chunks thrown off a contact onto flat ground: 10 of 45 lie at exactly y = 0 after 25 frames, and none goes deeper than 1.1 ft.
- `research/oracle.txt`: the P58 update line points at P61, plus a P61 block.
- opentennis has no particle system, so nothing was taken from it. Its restitution is the tennis ball's.

## What the original does

- **The update's collision** (`./kit fn 0x001556b0`): runs after gravity, when the particle's flags have 0x200 and the app's tick counter (`+0x374`, oracle `frame_counter`) is odd.
  - It sends a copy of the point to ground_query (`./kit fn 0x001dc960`, flags 0). The port's equivalent is `World::ground`: the heightfield at the point's x and z, whatever its y.
  - It computes `d = y − ground`. If `d < 0` and `vel.y < 0`, the velocity is scaled by Restitution (`./kit fn 0x00155bf0` is a vector times a scalar). Then:
    - if `d > −0.1`: `y = ground`, and flags 4, 0x200, 0x20000 and 0x80000 are cleared (`& 0xfff5fdfb`): no more moving, colliding, gravity or drag;
    - otherwise: `vel.y = −vel.y`, and the position is left as it is.
- **Which emitters collide.** Two places set 0x200 on an emitter:
  - The `.emt` reader (`./kit fn 0x0014e910`) sets it when `GroundLevel` is present. The key defaults to `FLT_MAX`, and only presence is tested. The value itself goes to another emitter type, at `./kit fn 0x001529d0`.
  - The vehicle manager's CollisionModel init (`./kit fn 0x003d73e0`) ORs 0x200 in after loading the `.emt` when the entry says `HasCollision`.
  - The Dust init (`./kit fn 0x003d67d0`) and the `ParticleCollision…` type (`./kit fn 0x003d7830`) read it too. Neither is ported (P60).
  - In `biplaneparticles.dat`, only Vehicle Chunks says `HasCollision = TRUE`. `wreckchunks.emt` has `Restitution = 0.60` and no `GroundLevel`.
  - The switch reader (`./kit fn 0x0014b3f0`) reads `TRUE` as on. The strings are at `./kit data 0x427130 0x30 --as hex`.
- **Wind** is not in the update. In the emit (`./kit fn 0x00150540`, at the `+0x234` test), an emitter with `Wind` adds its system's `+0x140` vector to the base velocity. The per-particle noise is added after.
  - The system's base constructor (`./kit fn 0x00155050`) sets that vector to 0, 0, 0, 1. Its subclass is `./kit fn 0x0031eaf0`, vtable `0x004593c0`.
  - A grep of every `+ 0x140` write and out-argument in the boot decompilation found no other writer for this class.
  - Live values, found by the vtable and read with `./kit state extract`:
    - `biplane.p2m2_SaveState.p2s`: the system is at `0x60bdd0`, with 208 sheet frames and 105 particles alive. `+0x140` is 0, 0, 0, 1.
    - `first_recording.p2m2_SaveState.p2s`: `0x604740`, 208 frames, 225 alive. Also 0, 0, 0, 1.
  - Many `.emt`s on the disc set `Wind = T`: the biplane's smoke and wreck dust, the tire dusts, mists, torches. None of them is pushed by anything.

## How it was proved

- **Rules:** the five new synthetic tests check each branch read above. The disc test checks every entry's collides/restitution/wind on free06's own files.
  - A temporary panic at the disc test's end proved it runs here and does not skip: `10 of 45 landed, lowest -1.1045519`. The synthetic chunk run printed `16 of 20, lowest 49.28` over ground at 50. Both were removed.
- **Wind is zero:** from two save-state RAM images, plus the static writer search (above).
- **Gate:** `./kit check` passed.
- **Live smoke run:** `openmx --headless --vehicle biplane --track free06 --frames 400` exits cleanly through the new `step_over` path. No contacts are fed, so no chunks are made (P63).

## Not verified / not 1:1

- **No live chunks:** no frame-by-frame or by-eye comparison of bouncing chunks against the original. The port's plane has no collision-model contacts until S48, so the wreck's chunks are still never emitted live (P63). Their bounce is proved only against the decompilation and on synthetic contacts with the disc's `.emt`.
- **The odd-tick phase is the fx's own.** Its first step is tick 1, so it tests. The original's tick counter has counted since boot, and how the PlaneFx's start lines up with it was not measured. The rule (every other tick) is the same; which ticks are tested may be shifted by one.
- **The ground is the terrain heightfield only.** It is the ride's `World::ground`, the same query the wheels use. Like ground_query with flags 0, it does not see props. Where the port's terrain has no cell, it answers −1e6, and a particle is never under that. Per the decompilation, the original leaves the point where it is, so `d = 0` and there is no collision either. Same result.
- **Host floats.** The particles are presentation and use host `f32`, as in P58. P61 also moved where the emitter's own velocity is summed (now before the noise, as in the original's emit) and added the wind there. The values match the original's operations, but not bit for bit.
- **The original's swap-on-death order.** When a particle dies, the original swaps the last particle into its slot and updates that one in the same pass without ageing it. The port's `retain_mut` keeps order and ages every particle once. This was already the case in P58: one frame of age on one particle per death, which is not visible.
- **`ON`/`OFF` and the `strtol` fallback** in `ini::switch` are read from the decompilation. No disc file uses them: the disc's switches are `T`/`F`/`TRUE`/`=1.0`-style numbers. Not checked against a running game.
- **`GroundLevel`'s value and `MatchTerrainContour`** (emit-time snapping to the terrain) are not ported. No emitter the port runs uses them: `biplanedust.emt` and `helicopterdust.emt` have `MatchTerrainContour` and belong to P60's HelicopterDust, and the geyser `.emt`s are not the plane's.

## Next

Nothing in P61. P63 (feed the collision model's contacts after S48) is what makes the chunks visible.
