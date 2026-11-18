# 1 — find quad in each image (FINAL)

Task MD116a. 2026-11-17.

## What I did

- `tests/atv_gather.rs`, `atv_search.rs`, `atv_spatial.rs` no longer pin MD37a's heap addresses. Each finds
  what it reads in the image, then runs on MD37a's `atv-r-spawn` and C28's `c28-atv-{r,s}-{spawn,air}`.
- How things are found (same in all five images):
  - player quad: lowest object with vptr `0x003ce250` at `+4`; idle quad is the other one (two in each image).
  - bikes: objects with vptr `0x003d0450` at `+4` (four), lowest first.
  - quad `+0xc4` list, `+0xd0` frame mesh, `+0xd4` sweep, `+0xd8` riding probe, `+0x1430` trigger probe,
    `+0x1434` wreck rays.
  - space tables: `*(*(probe + 0x20) + 4)`; tree at its `+0x28`, moving list at `+0x2c`. Checked by
    vtable (`0x003b6700` tree, `0x003b5510` list).
  - cells (atv_spatial): every object with the cell vtable `0x003b55b0` at `+4` (three), lowest first.
  - zones (atv_gather): the tree's collidables with a zone vtable (`0x003b6ec0`, `0x003b6a60`), put in the
    `.toy`'s order by matching each to the port's filed zone by centre and reach.
  - vehicle order in the tree: both quads, then the bikes by address. The tree test passes with that
    order in all five images.
- Two things the old tests took for granted that do not hold in C28's images:
  - The player's sphere is filed where its tree record says (record `+0x10`). In `c28-atv-r-air` that is
    9.5 ft from the gathering point (quad `+0xd90`), so filing at the gathering point put it in 4 slots
    instead of 2. The test now files at the record's centre. MD37a's image passes both ways.
  - A vehicle's list starts with the bound its last gather left. In `c28-atv-r-air` it is 10.0, so the
    walk's first call failed with a fresh list (bound 0). The image's bound is now taken as input.
- Moving list flag word (`+0xc`): 0 in MD37a's image, `0x30028` in C28's. The list's begin
  (`./kit dis -p atv 0x0018dae0 --len 0x28`) ors in bit 0 and keeps the rest, so the high bits are what the
  memory held. The test compares bits 0–1, the only ones the port models.
- atv_spatial: the tests of the image's own database (own moves, tree moves, moves on RAM) run on all five.
  The tests that build their own databases in free RAM still use only MD37a's image (see below).
- Took nothing from opentennis: this is test plumbing for MX vs. ATV images only.

## What the original does

No new behaviour. The images are SLUS-21104 RAM; the layout above comes from them.

## How it was proved

- `./kit check -- -p game-sim --test atv_gather --test atv_search --test atv_spatial`: 18 tests pass. With
  `--nocapture`, every C28 image prints its own lines (tree, collidables, vehicle walks, entries, trigger
  probe, search frames, captured rows). None skipped.
- C28's four give the same search counts as MD37a's (6001 frames, 1617 contacts; 14 captured rows,
  5 reproduced). So the S quad's trigger probe has the same four rays as the R quad's.

## Not verified / not 1:1

- Moving list flags: only bits 0–1 are compared. I did not check every reader of the list's `+0xc` for the
  higher bits (0x30028 in C28). Only the begin was read.
- Zones are paired to the `.toy`'s order by centre and reach, not by the original's own link from toy to
  collidable (I did not find one). So the zone test checks each zone's centre only as the key it was
  found by. Its other fields are still compared.
- Bike order (by address) is a guess. It is only confirmed by the tree test passing in all five images.
- atv_spatial's generated-database tests (they lay their own databases in free RAM at `0x01f80000`) run
  on MD37a's image only. They use only the image's code, which is the same ELF in all five. Free RAM
  there is all zero in all five images, but those tests were not run on C28's.
- atv_search's captured-contact test uses every capture's rows (race- and s-) on every image, as before.
  It does not pick the rows recorded with that image's quad. The counts match anyway.

## Next

None.
