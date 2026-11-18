# 1 — prop sphere bound (FINAL)

Task S324. 2026-11-18.

## What I did

- `race.rs`: `PropBound::query` now runs the bound rebuild's sphere branch instead of answering
  `None`. It takes a new `SphereRadii` (the node's own `+0x114` radius and each array child's); the
  box branch ignores it. The answer is no longer an `Option`.
- Two private helpers beside it: the transform's point call (origin into the accumulator first) and
  its unpoint call (transpose by lane moves, `vmulax` first).
- Tests 40–42 in `tests/race.rs` pass the radii. Test 40 now runs the sphere cases against the
  original instead of skipping them.
- Oracle entries: `prop_bound_rebuild`'s sphere branch, and a new `prop_sphere_merge`.
- opentennis: nothing taken. Its scene nodes are a different engine's, with no such bound.

## What the original does

- `./kit dis 0x001a17e0`, bit 4 set:
  1. Transform rebuild (`0x001b1750`).
  2. Radius 0. With bit 2 set: centre = `0x001b3060` (node, node+0xc0), radius = node+0x114.
     Without it the centre is a stack word nothing writes.
  3. Each array child (+0x11c count, +0x13c array) through `0x001b4110` (child, &centre, &radius, 0).
  4. +0xe0 = `0x001b3180` (node, centre). +0xf0/+0xf4/+0xf8 = radius, +0xfc kept. +0x118 = radius.
     Bit 6 is set when radius > 0 (`c.ole.s` and a branch over the set).
- `./kit dis 0x001b4110` (child, &centre, &radius, walk). It only acts when the child's bit 2 is set:
  - radius <= 0: the child's sphere replaces it (centre `0x001b3060` of child+0xc0, radius child+0x114);
  - else d = longest of |x|, |y|, |z| of (child centre − centre) (`vabs`, `vmaxy.x`, `vmaxz.x`), and
    radius becomes d + child+0x114 (`add.s`) when that is more. The centre does not move.
  - walk != 0 also walks the child's linked children (+0x104 first, +0x108 next). The rebuild passes 0.
- `0x001b3180`: (p − origin), then the rotation's transpose built with `pextlw`/`pcpyld` (plain lane
  moves), `vmulax`/`vmadday`/`vmaddz`, `xyz` only, so `w` is p's.

## How it was proved

All in `crates/game-sim/tests/race.rs`, through `./kit check`:

- 40 `the_prop_bound_query_is_prop_bound`: 10,000 generated trees against `0x001b44b0` with
  `0x001a17e0` behind it. The radii are above, at and under zero, with stray words. 1,686 of them
  were stale sphere rebuilds; all answers and kept words matched bit for bit (centre caveat below).
- A mutant that always grows the radius fails test 40, so the grow test is covered.
- 41 (race images): passes with the radii read from RAM. No scene prop on the eight images is
  sphere-bounded (0 counted), so this adds no sphere proof.
- 42 (sx01 finish line): unchanged result.

## Not verified / not 1:1

- **No box anywhere:** a sphere-bounded node where neither it nor any child has a box. The original
  unpoints a stack word nothing wrote. The port starts the centre at (0, 0, 0, 1). Test 40 skips the
  centre for those 410 cases and checks everything else.
- **No real prop:** the sphere branch is proved on generated trees only. None of the eight race
  images has a sphere-bounded scene prop.
- **Transform rebuild:** `0x001b1750`, which both branches and the point calls run first, is still
  not ported (as in S297). The tests set bit 0 so it returns at once.

## Next

Nothing left in S324.
