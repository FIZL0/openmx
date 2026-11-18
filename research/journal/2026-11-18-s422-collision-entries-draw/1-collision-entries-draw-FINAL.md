# 1 — collision entries draw (FINAL)

Task S422. 2026-11-18.

## What I did

- `game_sim::dust`: the `CollisionModel` and `ParticleCollisionConstraint` entries are no longer
  `Quiet`. Two new kinds, `Kind::Collision { zones }` and `Kind::Spheres { mask, zones }`, with
  `Entry::touch_update` as their update. It counts the draws of the emits it makes.
- Their inputs:
  - `Object`: the wreck bike's record and collider, read live, because the bike's node runs before
    the manager.
  - `Ball`: the ragdoll's bone collisions. They are held in `RiderDust::balls` one frame, because
    the body runs after the manager.
- `RiderDust::frame_with` takes both. `frame` is kept as the old call, with no touches.
- `Sim::dust_frame` fills them:
  - the object from `crash.bike` (`record.on_ground/on_object/contacts`, `collider.relative`);
  - the balls from `Rider::ragdoll` (a wreck) or `Rider::riding.actor` (on the bike), with each
    contact's `ground`, `scene`, `surface` and `point`, plus its bone body's `velocity`.
- The view cull, which the emits read:
  - `Emitter::cull` (`Cull { far, directional }`) and `Emitter::culled`;
  - `Emitter::view_test`, run on every entry after the manager pass;
  - `Dust::view`, the camera input (`View { at, forward }`). With none, the camera is the bike's
    point with no forward row.
- `emit_at`: just-on, then culled (no draws, points kept), then off, then the speed test, which is
  the emitter pass's `gate`, now shared.
- The hash walks `culled` and the held balls.
- The C33 seed (`tests/ai/race4.rs`) fills the new fields (cull from the image, `view: None`,
  `balls` empty).
- `tests/ai/draws.rs`: `through`, `whole_frame`, `neutral_pad`, `render_setup` and `render` are
  now `pub(crate)`.

## What the original does

The details are in `research/oracle.txt` under S422 (`./kit dis 0x003d75d0 --len 0x200`,
`./kit dis 0x003d7a30 --len 0x200`, `./kit dis 0x0014d8c0 0x0014daa0`).

- The manager switches the two kinds by the wreck flag alone (no tyre).
  - Each rider has two collision models (Disable 0x15, MES 3 and 15) and two sphere entries
    (0x15, mask 0x1fc).
  - There is also the foot dust (0xd, mask 0x3 = the two foot collisions, MES 30).
  - So the 0x15 entries live only in a wreck, and the foot dust only on the bike.
- Both updates call emit on their emitter from inside, once per contact. A whole-frame walk
  logged every emit on those emitters: all came from the two updates (ra 0x003d776c and 0x003d7c08).
  The emitter walk never emits them.
- Collision model:
  - Flags = wreck | ground | object | none, from the collision object.
  - If Disable lets the flags through, the emitter goes on. Each contact whose zone byte is not
    0xff emits: `prev = point − relative·dt` (`.xyz`), `pos = point`.
  - Otherwise the emitter goes off.
- Sphere entry:
  - The emitter is on if Disable lets the wreck flag through, else off.
  - Then each sphere in the mask with a zone and with ground/scene flags that Disable lets
    through emits: `prev = point − bone velocity·dt`.
- The emit gate (emit_timing), with no node: just-on comes first (prev = pos, no draws). Then the
  **view cull**: `+0x248 && *0x0041f788` means no draws, with the points kept. Then the mode-0
  on/off. Then the speed test.
- `+0x248` is written once a frame by the emitter's view test (vt +0x94, 0x0014d8c0), after the
  manager.
  - It is set to 1 at the frame's first call.
  - It is cleared when the point is within FarCull (800) of the display camera and either in
    front, or behind but within DirectionalCull (200).
- An emitter switched on carries the last frame's answer, made at its stale point. In S325's wreck
  the bike's emitters sat 5,938 ft away, so in the frame they switched on:
  - the first contact only took its point;
  - the second was culled.

  This was the only difference left before the cull went in (4 updates over ticks 2616–2617).
- Frame order: the wreck bike's collision node runs, then each manager, then each rider body.

## How it was proved

`crates/game-sim/tests/ai/wreck_dust.rs`, `s422_collision_entries_draw_as_the_original`:

- Setup: the original's whole frames (C33's harness) from `capture/s138/s325-nose76-2604.ee.bin`,
  45 ticks. The rider lands on the nose at 2615 and is thrown.
- Before every collision-model and sphere-entry update, the port's entry, emitter and inputs are
  seeded from RAM and `touch_update` runs. The original then runs.
- **372 updates** (158 collision-model, 214 sphere): the rands drawn and the emitter's switch, pos
  and prev (bit for bit) all equal the port's.
  - 54 of them emitted, with 228 draws.
- Any emit on those emitters from outside the updates fails the test. There were none.
- After each frame, `view_test` on the display's camera equals every collision emitter's `+0x248`:
  **990 tests**, 583 culled.

## Not verified / not 1:1

- **The Sim end to end through a wreck is not compared with a capture.** The proof seeds each
  update from the original's RAM.
  - The Sim's own inputs (`crash.bike.record`/`collider`, the ragdoll contacts and bone
    velocities) come from S69/S53-era ports that were proved on their own.
  - But no Sim run of `RiderDust` through s325's wreck was set beside the original's draw count:
    the C33 seed only seeds non-wrecking riders.
  - Follow-up S476.
- **The ball order equals the feed's sphere list.** Assumed: `contacts[k]` ↔ `ragdoll.collisions[k]`
  ↔ feed sphere k. This fits the masks (0x3 = the two foot collisions, `FOOT_COLLISIONS = [1, 0]`).
  The feed's sphere pointers were not checked against the actor's collision list.
- **A disabled collision (a held foot while riding) is passed as a ball.** The original's feed
  pointer may be 0 for it, or its flags may be stale. Not seen: the foot dust never emitted in the
  45 ticks.
- **The camera.** The shell never builds a `Dust` (only the C33 test does), so `Dust::view` is
  never fed. With none, the Sim measures from the bike's point with no forward row. In the original
  the camera is ~20 ft behind and looks forward, so a stale point between 200 and 800 ft behind the
  camera answers differently. Exact only with the shell's camera.
- **The view test's own call.** It runs from the emitter walk, once a frame; which point it is
  given was not read. The port uses the emitter's point after the manager, which matched all 990
  tests. Its fade value (`+0x254`) is not ported: it is presentation only.
- **Zone indices outside 0..16** (a surface of −1, or a sphere surface byte ≥ 16) read the entry's
  neighbouring bytes in the original. The port counts them as zones with particles (those bytes are
  pointer bytes and never 0xff). The image's surfaces were only 0 and 13.
- **The group switch** (`0x0014e330` when the zone's group differs) is not ported: it picks the
  particles' look, not the draws.
- **The emitter pass's own entries (exhaust, generic, spray) also go through the view cull** in
  emit_timing, and S328c's `Emitter::frame` does not read it. Whether the bike's emitters have
  `UseViewDependentCull` set was not checked. Follow-up S477.
- The pause byte (`*0x0041f770 + 0x325`), the delay and the modes are not read, as in S328c (all
  0 here).

## Next

Land it (`./kit done S422`).
