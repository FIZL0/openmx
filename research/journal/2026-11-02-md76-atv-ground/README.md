# MD76 — Desert Oasis's ground for the ATV

2026-11-02. Task: `./kit plan MD76`.

MD76a (`2-record-byte-MD76a.md`): the record byte now reaches `atv_wheel::suspension` and `quad::Steps`. This is proved with the suspension running the original's own query over Desert Oasis on all 12,200 captured wheels. The collision sweep is split off as MD76a1 (the collidables and gather) and MD76a2 (the search).

MD76a1 (`3-collidables-and-gather-MD76a1.md`): MC02's meshes and plants in the original's quadtree,
and the gather that fills the quad's nearby list (quad `+0xc4`), equal to the original's on 9,930 probe centres.

MD76a2 (`4-search-MD76a2.md`): the quad's wheel-ray search (`ContactCall::Collide`) over that list, its
record equal to the original's bit for bit on 6,001 frames (1,617 contacts on all 120 meshes) and on 136
frames blended from MD37a's swept-contact rows.

MD86 (`5-contacts-MD86.md`): `quad::Steps` answers `Call::Contacts` over Desert Oasis (`atv_search::Oasis`).
The original's distributor and search run unhooked, and the quad, the body and the wheels equal the port's on
763 frames.

`game_sim::atv_ground` ports two SLUS-21104 functions: the ground query the quad's wheel suspension
asks under each probe end (`atv_ground_query`), and the terrain's cell reader under it
(`atv_terrain_cell`). It runs over Desert Oasis's map as the game loads it. `tests/atv_ground.rs` runs
the original's query off MD37a's `atv-r-spawn.ee.bin`, with nothing hooked, and compares the answers
with the port's.

## The map

Desert Oasis Free Ride is `Env/MC02`. The terrain is the member `Env\mc02\MC02.ps2.trn` of `mc02.res`.
I copied it from MX vs. ATV's extracted disc (`openmx-mods/extracted/mxvatv/Env/MC02/`) into
`context/mxvatv-ps2/md37a/` (git-ignored). Cell (628, 732) in the image's ring has the file's heights,
which is how the map was identified.

The terrain object the four wheels hold (wheel `+0x1e0`) is `0x0184c7d0` in the image. Its words are in
oracle.txt's MD76 block. `Terrain::of(map)` works all of them out of the file: box (0, 5760) on `x`
and `z`, margin 0.01, 1/4.5 samples per foot, height scale 4.5, not coarse, split one way. The first
test checks them bit for bit against the image.

## What differs from MX Unleashed's query

The query is MX Unleashed's (`contact_search::ground`) built again:

- `x` and `z` are held a margin inside the box, `max(p, min + m)` then `min(., max - m)`, before
  they turn into samples.
- Both height sums are FPU accumulator chains, `adda 0, h0; madda u, (h1 - h0); madd v, ...`.
- The cell reader returns normals as `(n0·vs/127, n1/127, n2·vs/127, 1)`, where 1/127 is a `div.s`
  each time. A flattened node's height is `hs · (adda 0, base; madd vs, raw - base)`.
- The terrain object's fields moved (see oracle.txt).

The original keeps the last ten cells it read in a global ring (`0x00541550`). The ring is keyed by
the sample alone, not the step and not the terrain. With one terrain loaded and a fixed step, a hit
returns what a read would, so the port has no ring. The test runs the original with the ring warm on
some calls and cold on others (`+0x9e0 = 0`). A cold start clears the keys of 32 entries, though the
count is 10.

## The proof

`the_atv_ground_query_is_the_originals_along_md37as_rides` asks both about 29,450 points:

- every wheel's probe end (wheel `+0x160`) in every row of MD37a's 15 captures: 12,200 points;
- as many again scattered ±12 ft across and ±20 ft up and down around them;
- 2,000 points past the box's edges;
- 3,050 points on cell midlines and corners, where two corners tie for the nearest (1,714 ties).

On each call the test may:

- flip the coarse flag (10%) or the parity split (15%);
- start the ring cold (30%, and always after a flip);
- leave out the normal or the record buffer (10% each).

It compares the point, the normal (pre-filled with a marker) and the record byte (pre-filled with
0xee) bit for bit. It also checks that the original wrote only the buffers, the ring, the terrain's
`+0x9e0..+0x9e8` and the stack. Every path counted was seen more than 1,000 times: both splits, each
with both height branches, coarse, held in the box, ring warm and cold, ties, and no normal and no
record asked.

Mutation checks:

| Mutant | Result |
|---|---|
| Hold removed | red |
| `d0 < d3` → `<=` | red |
| Normal `ny·k` → `ny/127` | red |
| Record `<` → `<=` | red (after the tie points were added; green before) |
| Height accumulator chain → plain `h0 + u·(h1 - h0) + v·(h2 - h0)` | green |

The accumulator mutant survives because, in ps2-float, the chain differs from plain sums only in the
sign of a zero ground. No Desert Oasis height comes out −0, because the reader starts every height from `+0`.

How it was checked: `./kit check -- -p game-sim --test atv_ground` (2 passed), the mutants above,
`cargo check -p game-sim --tests` (no warnings in the new files), and `./kit audit`. The audit's only
leaks are two lines of plan/DONE.md that this task does not touch.

## Not verified / not 1:1

- **The collision sweep in the brief is not done.** That is the quad's wheel contact search,
  `ContactCall::Collide` in `quad::wheel_contacts`, over Desert Oasis's collision objects. It is filed
  as MD76a.
- **The record byte is not wired into the suspension.** The suspension passes the wheel's `+0x110`
  as `t0`, but `atv_wheel::suspension`'s ground closure is `FnMut(Vf) -> (Vf, Vf)` and drops it.
  `Terrain::ground` asks with no record. Wiring it is part of MD76a.
- **No-cell path untested.** Inside the box, Desert Oasis has a cell at every sample, so the UP
  answer is never compared. Past the edges, points are held onto edge cells.
- **Normal-table path not ported.** Normals from the 256-direction table, used when `0x00541518` is
  set, are not ported. Nothing in the ELF writes that flag.
- **The ring is not modelled.** With two terrains, or one asked both coarse and fine, the original's
  ring would hand back a cell read for the other. The port reads afresh.
- **The accumulator-vs-plain-sum difference is unproved.** On this map it never shows (see above).
- **NaN points.** The original reads a stale register for the record; the port leaves the record
  alone. Not compared.
- **The test's points are probe ends from the captures**, not contact points the game actually asked
  about during the ride. The query is a pure function of the point, so this costs nothing.
