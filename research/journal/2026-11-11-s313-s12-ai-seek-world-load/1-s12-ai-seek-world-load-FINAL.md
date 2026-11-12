# 1 — s12 ai seek world load (PART)

Task S313. 2026-11-11.

## What I did

- `ai::seek::load` (crates/game-sim/src/ai.rs) builds a map's seeker `World` from its `hit.dat`, the hit
  database and the hit-set database `hit.dat` names (`ai::seek::databases`), the riders' owners, the slot
  count, an `open(player, Zone)` answer and the ground. It fills:
  - `jumps`, one per hit in database order;
  - `targets`, 1:1 with the jumps, each with `done` false per slot;
  - `groups`, one per start zone, with their hit-set list, point, in-play flag and two open flags;
  - `sets`, one per owner, with picks and course `None` (so `build` chooses later) and `sequence: None`.
- New `ai::seek::{Activity, Zone, OPEN_PLAYERS}`.
- `phy::Section::texts`: every value of a key, in file order. The original reads a hit set's
  `HitID`s and the progression lists' `HitSetID`s this way, with a running cursor.
- The test is `s313_free09_world_loads_as_the_image_holds_it` in crates/game-sim/tests/ai.rs.
- The addresses are in research/oracle.txt under `ai_seek_root_ctor` … `ai_seek_zone_open`.

## What the original does

- `./kit fn 0x3a9680` (the load), called with the root, the hit.dat reader and the slot count:
  - It allocates the rider sets. Riders take them as they join (`./kit fn 0x3a9d60`, `0x3a6600`).
  - It reads `[HitData]` and puts the track folder in front of the two database names.
  - It reads the two progression lists.
- The hit database (`./kit fn 0x323b00`, legs read by `0x322800`):
  - It reads `TotalHits` hits.
  - A HitID of 0 or none takes `NextHitID`, which then counts on.
  - Each hit reads three `[HitZone Start|TakeOff|Landing n]` sections: Position, LookVector, Scale,
    TargetVelocity, TargetPreload and TargetPosition.
- The hit-set database (`0x323d30`) works the same way: auto ids, and repeated `HitID` keys.
- The targets (`0x324610`):
  - One per hit, with a progress record per slot (done = bit 2 of the u16 at +0x20, zeroed by the
    record's constructor).
  - First, each hit's three points are put on the ground. Only the height changes (`0x1dc960`).
- The lists (`0x324b90`): a HitID maps to the first target whose hit has that id (a 16-bit compare).
- The zones (`0x326330`, each read by `0x3259b0`):
  - `[ASZ n]` for n = 1..ASZCount.
  - Position is put on the ground.
  - Type (case-insensitive) sets +0x78: Hit 1, Run 2, Race 3, Stunt 4.
  - HitSetID (default -1) picks the first list with that id. It is a 32-bit compare, so -1 never matches.
  - +0x70 is the zone's last place in its progression list.
  - So `build`'s "in play" test (+0x78 == 1) means "a hit zone".
- The open bytes (+0x44): two players. They are set later, from the save profile
  (`0x330f10..0x331030`), not from the disc.

## How it was proved

- Save-state comparison: recording2's free09 image (s2). The input files come from the disc's
  `Tracks\free09_2.res` (hit.dat, free.hit, free.set). Points are put on free09's terrain through the port's
  `Place` ground.
- What the test compares, all bit-exact:
  - all 106 jumps × 3 legs × (velocity, aim, point, facing, learnt);
  - all 106 targets' jump and 5 done flags;
  - all 12 groups' lists, by target, including group 8's repeats;
  - their points, in-play word and both open bytes' bit 0;
  - all 4 sets' owner kind and player, with no picks or course.
- The non-zero take-off velocities and preloads in the image (hit 1: v (-11.02, 0, -109.45), learnt 50) come
  from free.hit itself, not from learning.
- Mutant: with the ground snap turned off, the test fails at jump 0's take-off point (y off by 1 ulp). So the
  port's ground query gives the original's heights bit for bit at these 330 points.

## Not verified / not 1:1

- **Open flags.** They are an input (`open(player, Zone)`), not read off the disc. The original sets them
  from the save profile's progression bytes. The test passes "player 0, first place of each progression",
  which matches this save. Profile → open is not ported (follow-up S315).
  - Only bit 0 is modelled; the original's bits 2 and 4 (shown, and the second flag) are not.
  - The hit-zone condition's extra test (`0x3aa780`) is not traced.
- **Owners.** They are runtime riders and are passed in.
  - The test takes kind, player and position from the image. kind and player are compared; position goes
    straight through.
  - `hit_freeride` is passed as false. It is a vtable answer and was not checked against the image (`build`'s
    own oracle covers it).
- **Slot count.** It is an input; the test takes it from the image (5).
- **Not kept**, because the seeker never reads them: each leg's Scale (+0x40), a zone's LookVector, Scale and
  AIDifficulty, `StringID`, and the zone number at +0x74.
- **`EnableAllHits`** (root+0x58) is not modelled. free09 comments it out.
- **Course and sequence are `None`.** The image's set +0x114 is 0, and the sequence word has no writer in the
  ELF.
- **The rider sets.** The original allocates `slots` sets and fills them as riders join. The port makes one
  per owner it is given, so the image's empty fifth set has no counterpart.
- **Parsing edge cases.** A missing `[HitZone …]` section or key keeps the fresh record's values (the
  defaults in `leg`). Only free09's well-formed files were seen.
  - The base-10 u16 id reader and the progression lists' `int` reading (T/F words, base 0) follow the
    disassembly. No oracle was run on odd text.
- **Not wired in.** `load` is not called from the Sim or the shell yet; the seeker worlds there are still
  built by hand (follow-up S314).
- **Other games.** opentennis has no counterpart to take or follow. The PC build was not read.

## Next

The work is done and committed on task/S313; only the tick is missing. `./kit done` refused: the gate is red
on this base (044df50e2) in `mxvatv_car_frame::all_eight_pieces_are_mx_unleasheds` ("preload: fired,
degenerate seen only 19 times"). That is MD132's red, which this task does not touch. Main fixes it in
078736f3d (F31b5l: the eight-piece run goes 6,000 frames). Once that is merged into the branch, run
`./kit done S313 "S313: ..."`; it should go green and tick. Follow-ups: S314 (wire the load in), S315
(open flags from the profile).

## Closed 2026-11-11

The MD132 base fix (F31b5l, 078736f3d) is on this branch now. `./kit done` ran the gate green and ticked
S313. No code changed in this session.
