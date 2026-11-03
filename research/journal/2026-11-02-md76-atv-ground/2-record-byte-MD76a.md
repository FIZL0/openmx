# 2 — the record byte through the suspension (MD76a)

2026-11-02. Task: `./kit plan MD76a`.

MD76a had two parts. One is done here: the ground query's record byte now reaches the suspension. The
other, the quad's collision sweep (`ContactCall::Collide`) over Desert Oasis's collision, is too big
for one task, and it is split off as MD76a1 and MD76a2 (below).

## The record byte

The suspension hands the ground query the wheel's `+0x110` in `t0`. The query writes the surface of the
nearest corner there (MD76). The port's ground closure was `FnMut(Vf) -> (Vf, Vf)` and dropped it.

- `atv_wheel::suspension`'s ground closure is now `FnMut(Vf, &mut u8) -> (Vf, Vf)`. It is handed
  `More::record`.
- `quad::Steps`' ground is the same closure type.
- `atv_ground::Terrain::ground(at, normal, record)` asks the query with the record. So on Desert Oasis
  the closure is `|at, r| terrain.ground(at, Vf::ZERO, r)`.

## The proof

- The two hooked oracles now have the ground query hook write a generated record byte, or leave it:
  `the_atv_wheel_suspension_is_the_originals` and `tests/atv_quad.rs`'s `Steps` tests, including
  `the_quads_suspension_is_the_wheels` and `the_quads_force_body_runs_the_bikes_steps`. The wheel's
  `+0x110` word is compared, and its upper three bytes hold a marker, so a wider store would show.
  "Missed under a swept contact" now means the miss changed the record from what the last query left.
- New: `the_atv_wheel_suspension_on_desert_oasis_is_the_originals` (`tests/atv_wheel.rs`). It runs
  SLUS-21104's suspension with the ground query **unhooked**: a hook logs the call, then runs the
  original's own query on the image's Desert Oasis terrain object (`0x0184c7d0`). The port asks
  `Terrain::of(MC02.ps2.trn).ground`. It covers all 12,200 wheels of MD37a's fifteen captures, each
  with its captured mount, probe, probe length, ground normal, contact and record, and its quad body's
  position, orientation and centre of mass. To reach the branches the captures rarely take:
  - a third of the rows get a different ground normal, so the ray aims again;
  - half get a record left from elsewhere, so the query writes over it;
  - half get a swept contact under the mount, with terrain_wins either way;
  - a sixth are lifted 0.5–3 ft, so the ray misses, and a sixth are sunk up to 1 ft.

  The wheel region is compared word for word, and so is the body. Calls are counted. The original may
  write only the regions, the stack, the query's ring and the terrain object's ring words. Each of
  these paths is seen at least 30 times: missed, missed under a swept contact with the record cleared,
  swept kept, ray hit, terrain won, aimed again, and record changed.
- Mutations, each reverted:
  - the suspension handing the query a copy of the record: both suspension tests go red;
  - `Terrain::ground` asking with no record: the Desert Oasis test goes red.

How it was checked: `./kit check -- -p game-sim --test atv_wheel --test atv_quad --test atv_ground`
(32 passed before the new test, then `--test atv_wheel` 5 passed with it). `./kit audit`: the only leaks are the same two old plan/DONE.md lines MD76 found, which this task does not touch.

## The collision sweep: what was found, and the split

- The distributor calls `collide_against_search(quad +0x1430, quad +0xc4, 1)`. In SLUS-21104 that is
  `0x001151c0`, which has the same shape as MX Unleashed's `world_search::search_nearby`. The search
  object's vtable is `0x003b6c20`:
  - `+0xb0` clear: `0x001bbbc0`;
  - `+0xb4` narrow: a thunk that stores `+0x184` in a global and jumps into the ray probe's pair at
    `0x001bc880`;
  - `+0xc8` record clear: `0x001bd180`;
  - `+0xd0` prepare: `0x001bc6e0`.
- The nearby list at quad `+0xc4` (`0x01627a40` / `0x01627c20`) is **empty in all four of MD37a's
  RAM images**: no prop is near the spawn or the thrown jumps.
- Over the captures, wheel `+0xd4` (the distributor's touch) is set in only 9 rows. They are all by the
  palm ahead of the spawn: race-jump at vsync 35853/35855, and race-wheelie-launch at 35805 and
  35851–35863. In those rows the owner is 0 and the record byte is 0. The last rear node, quad
  `+0x143c`, is `0x00ff9d00` / `0x00ff6cd0`. These are two collidables of the mesh vtable
  `0x003b6d00`, with reaches 23.5 and 28.6 ft.
- The record bytes over all captures are 0 or 6. They are Desert Oasis's surfaces as the query writes
  them.

So porting `Collide` faithfully needs MC02's collidables built from `mc02.res` as SLUS-21104 builds
them, its broad phase, and the gather that fills `+0xc4`. MX Unleashed needed several tasks for the
same (S44, broad_phase, broad_build). It is filed as:

- **MD76a1**: the collidables, the broad phase and the gather, proved by the list by the palm;
- **MD76a2**: the search and its record into `quad::wheel_contacts`, proved off the image with the
  palm in the list and on the nine touching rows.

The brief also wants MD37a's rides to run "with nothing hooked". That needs MD75b's calls (engine, rev
limit, gravity, air drag and the rest), which are not ported yet. MD76a2 says so.

## Not verified / not 1:1

- **The collision sweep is not ported** (MD76a1, MD76a2). `quad::wheel_contacts` still takes its
  contacts from its caller.
- **No whole ride runs unhooked.** The Desert Oasis test runs the suspension alone, one call per
  captured wheel. It is not a step of the ride, and the rows do not feed each other.
- **The normal buffer's prior contents.** The original's query writes the normal into the suspension's
  stack buffer. Where there is no cell, it writes UP and leaves the point. The closure hands
  `Terrain::ground` a zero normal for "what was there". On Desert Oasis every point is held inside the
  box onto a cell, so this never shows. On a terrain with holes, what the stack held would matter, and
  it is not modelled.
- **The ring stays warm** through the new test (the image's ring, then each call's). MD76 proved the
  answer does not depend on it.
