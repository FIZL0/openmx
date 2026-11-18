# 1 — quads among quads

Task S481. 2026-11-18.

## What I did

- `quad::step_together` steps several quads in the original's frame order:
  1. every quad's controls;
  2. every record cleared;
  3. every record's pass, each striking the others' hulls, with the hulls' records moved in so the
     mirrors (kind 1) land on them and stay there;
  4. every held list refilled from its whole list (`Held::refill_among`), with the other quads'
     records and bodies;
  5. one world step over all quads (`Frames`, a `biplane::Dynamics` with each quad's frame as a list
     entry; each force call lays the other quads' bodies into the held list and writes the pushed
     ones back);
  6. every quad's tail, then its wreck test.
- `quad::Among` carries each quad's ride, controls, trigger segments, name, hull and `trees`: the
  stored ray trees of its parts (the riding `.col`'s). The pass strikes with the tree whose rays are
  the active part's, else builds one as before.
- `Knockable::category` (`with_category`): the struck collidable's category word. `Ride::struck` now
  gives the hull `RECORD_CLASS` (0x3012, now `pub`).
- `step_quads`, `struck` and `take_pushed` are left as they were (the MD98a test still uses them).

## What the original does

- Sim frame (oracle `# F31a`): every record's clear, then every record's pass, then the world step,
  then the tail. A pass's mirror goes onto the struck record's list, which the next clear empties,
  not the next pass.
- What a pass strikes (oracle `# C38`, the S481 lines): the record's part (`+0x200`) is a ray probe
  whose tree is `250R_ATV_ChasisM.col`'s as stored. Its result record holds exactly the record's own
  kind 0 entries. Every hit is on the other record's hull (`*(rec+0x208)`): a mesh (the
  `ATV_R_ChasisF.col`), its record the other's, category 0x3012, mask 0.
- The pass runs at the frames from the frame before (37521): the world step moves the parts after it.

## How it was proved

- `quads_touch::the_passes_strike_the_other_quads_hull_as_the_originals` (C38 save-state images
  `c38-37521` and `c38-37523`):
  - each part is a ray probe; its hits are its record's kind 0 entries, word for word; each struck
    collidable is the other's hull, a mesh, the other's record, category `RECORD_CLASS`, not masked;
  - the hull in RAM equals the port's `ATV_R_ChasisF.col` (tree, facets, vertices); each part's tree
    and rays in RAM equal the riding `.col`'s; the frames are their nodes' worlds;
  - the port's passes (`RecordAmong` over `record::pass`), 0x01a511b0's then 0x01aa5020's, give both
    37523 lists bit for bit (all 4 entries each, both kinds); the other order does not.
- `mxu::two_quads_step_together_on_free09` (free09 disc world):
  - one quad alone in `step_together` matches `Ride::step` bit for bit (whole wire snapshot) over
    `STAND + 20` frames;
  - two quads meet: each list keeps the other's mirrors (28 and 25 frames), the held list holds them
    with the other's body as their other side, the gravity call is answered, no body leaves the world.

## Not verified / not 1:1

- The C38 proof runs with the records' sweep off and on a flat world 10000 ft down: only the quad
  hits are compared, not the world hits (none were filed at 37523).
- With a built tree (no `trees`) the hits file in another order than the original's; the shell must
  hand the riding `.col`'s tree. The wreck rays' stored tree is not handed or checked.
- The order of the world step across quads (the slice order as the body list's order), and of the
  tails and wreck tests, is not captured: no capture of a whole joint frame was compared.
- `Struck.object` is the record's name, not the collidable's address.
- Bales' and bikes' hulls keep their own category (`HULL_CATEGORY`); only the quad hull's was proved.
- The shell does not call `step_together` yet: it still steps quads one at a time (N29's wiring).
- Nothing touches the snapshot wire: the trees are not kept on the ride.

## Next

- Wire `step_together` into the shell's lanes, handing each quad the riding `.col`'s tree.
