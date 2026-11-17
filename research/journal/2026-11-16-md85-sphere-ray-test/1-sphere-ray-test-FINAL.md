# 1 — sphere ray test (FINAL)

Task MD85. 2026-11-16. Follows MD76a2 (research/journal/2026-11-02-md76-atv-ground/4-search-MD76a2.md).

## What I did

- The sphere ray test was already ported and proved by F46d (`atv_part::sphere_reaches`, the probe
  tree, the node box and the segment, 40,000 oracle cases each). Nothing new to port.
- The quad's search (`atv_search.rs`) now uses it. When the search asks a plant group for a sphere
  part, the part is stood at the plant with the F54 plant stand (`atv_plant::stand`). The pair then
  sets the record's "first" to 1 and runs the sphere test, as the bike's part search does.
- `Plants` keeps each species' sphere parts as the loader leaves them (`atv_search::sphere_part`):
  point = the solid's `at` with `w` 1, identity frame, radius from the file, radius² its square. It
  also keeps the stand's four scale words (height usual/least, width usual/most).
- `tests/atv_search.rs`:
  - the probe test checks every image sphere part's point, frame, radius and radius² against
    `sphere_part`, and the eco's cell (`+0xc4`) against the port's;
  - the search test now needs the hit bit equal on every frame (the allowance is gone). It also
    asserts that over 100 frames strike a sphere alone.
- Nothing taken from ../opentennis (MX vs. ATV code it does not have).

## What the original does

- `./kit dis -p atv 0x001095d0`: the sphere pair, F46d's port. Hit bit only, no contact.
- `./kit fn 0x0046d3c0 -p atv`: the plant stand (F54). A sphere's frame position becomes the plant's
  cell × the eco's unit, then it is refreshed (centre = frame × point, reach = radius).

## How it was proved

- `./kit check -- -p game-sim --test atv_search`: 3 passed. The search test runs the original's gather
  and search in the MD37a image over 6,001 generated frames. Record and hit bit are bit-equal on all of
  them; 131 frames strike only a plant's sphere (count read with a temporary assertion), the same 131
  MD76a2 had to allow.
- The image's 9 sphere parts match `sphere_part` word for word.
- `./kit check -- -p game-sim --test atv_quad`: 120 passed.

## Not verified / not 1:1

- **A sphere solid with a non-zero `at`**: every MC02 sphere's `at` is 0, so the point's `xyz` taken
  from `at` is unproven. Only `(0, 0, 0, 1)` was seen.
- **Hidden and turned plants**: the stand is called with flags 0 (the port's groups do not carry the
  plant's `+0x16` byte). A hidden plant would stand no part in the original. Desert Oasis has none
  that shows: the hit bit matched on all 6,001 frames.
- **The stand's two scale words** are computed but not compared here: nothing in the ray pair reads
  them. F54 proved the stand itself.

## Next

Nothing; MD85 is done.
