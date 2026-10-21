# 1 — put back clearance (FINAL)

Task S75. 2026-10-21.

## What I did

- `research/oracle.txt`: a block after `put_back_gate` describes `put_back_solid` in full. New names:
  `put_back_refile`, `put_back_ring_filter`, `put_back_over_filter`, `bundle_sweep_object` and
  `put_back_mask`.
- `World::crowded(centre, radius)` (default: no) in `crates/game-sim/src/world.rs`. `ride::clear` refuses a
  ring when the world says it is crowded, as well as when a ring probe hits.
- `crates/game-sim/tests/ride.rs`, `TheOriginalsTerrain`:
  - `solid` and `crowded` are now answered by the original's `put_back_solid`, on the bike's own bundle
    (the port's bundle is written into it) and the bike's own search list.
  - Before a ring of more than one probe, the bundle is re-filed when the cache's reach is below the
    distance moved, as `put_back_clear` does.
  - `crowded` asks the same query with zero probes.
- `the_put_back_record_is_kept_bit_for_bit` now holds the held ticks too (`Refusals`). Each held tick is
  tried from the same probe and heading candidates as a kept tick. It counts as matched if one candidate
  is refused or keeps the record on its exact bits. None may move. A refusal counts as a mesh refusal
  when the same probe over `NoSolid` (ground only) would have been kept.
- New test, `the_put_back_clearance_refuses_where_the_original_does`: 5000 generated probes per track
  around the objects the query sweeps. The original's `put_back_keep` and the port's `keep_back` run from
  the same probe, heading and record.
- The RAM images and captures for these tests were copied from slot s2 into `context/s66/`
  (`free06.eeMemory.bin` is s2's state slot 06 and `free09.eeMemory.bin` is its slot 07).
  `back_run` prefers these over the state slots: slot numbers belong to each emulator copy, and the main
  context's `.07` is a different state.

## What the original does

`./kit fn 0x0027df50` (put_back_solid). The arguments are the bike, its search list `*(bike+0x148c)`, a
filter, the bundle `*(bike+0x1484)`, a normal out, and `y` in/out. The query walks the search list's
(object, distance) pairs and looks only at those closer than the bundle's radius:

- **Type 2** objects with `+0x31 & 1` set, and `+0x84` not masked by the word at `put_back_mask`. The
  bike itself is masked off by `0x10000`. For each one the query calls `bundle_sweep_object`
  (`./kit fn 0x0011c210`). If that returns "met", `y = max(y, record point0.y)` and the query copies that
  normal. If any probe flag `1..n-1` is set, the query refuses.
- `bundle_sweep_object` clears the record's flags, fractions (to −1) and tally **for each object**, but
  never its points or normals. So the record after the query holds the last object's answer only.
  Reading it at the end loses hits; the per-object answer has to be read as it comes.
- **Type 3** placed groups refuse when any part's cell is within 20 ft. No probe is involved, so the
  refusal has no flag in the record.
- In race mode 1, checkpoints refuse.
- The overhead probe (count 1, radius 1000) ignores the return value and reads only `y`.
- `put_back_clear` re-files the bundle through `put_back_refile` (`./kit fn 0x001ab310`) when
  `*(bike+0x13a4) < *(bike+0x13a8)`.

## How it was proved

Function oracle in the interpreter, against the save states' RAM:

- **Captures** (`context/s66/free06-ride2/down/downchord.txt`, `free09-ride2/down.txt`):
  - Every kept tick is still bit-exact (434 + 463 + 292 + 380 + 406 ticks), now with the original's meshes
    answering.
  - Held ticks: 103/74/66/125/114.
  - 0 refused by a mesh, 244 by the slope, 238 kept in place, 0 moved.
  - The mesh query was asked 19,148 times and met nothing. The objects it sweeps are hundreds of feet
    from both tracks' paths.
- **Generated probes**: 10,000 (5000 per track, around 7 swept objects on each).
  - free06: 3370 kept bit for bit, 1581 refused with something solid in the way, 49 by the ground.
  - free09: 3221 kept, 1706 refused by something solid, 73 by the ground.
  - Kept or refused agreed on every probe.
  - The query refused 272 and 248 rings with no probe flag. Each of them was a ring `crowded` refused
    too, and the test holds the two counts equal.
- The interpreter's `y` and normal equal the highest point any object's answer left, on every query. The
  test asserts this.

## Not verified / not 1:1

- **The game's own `World` is not the original's query.** `Place::solid` sweeps the port's scene props,
  and `crowded` is always no there. Which meshes the original's search list holds, its flags and mask,
  and its placed groups are matched only in the test world, from RAM. That is follow-up S80.
- **Checkpoints in a race.** Both states are freeride, so the race-mode branch never ran.
- **Captured refusals by a mesh: none.** No captured tick came near a swept object. Real-ride mesh
  refusals rest on the generated probes alone.
- **Held ticks are matched leniently.** One candidate source and heading out of up to 16 refusing, or
  keeping the record in place, is enough, as S66's kept ticks were matched. A refusal is not checked to
  come from the same candidate the original used.
- **The generated probes' range.** The probes sit 2–8 ft over the ground, within the radius + 50 ft of an
  object, with random headings and a pitch of ±0.2. Probes further out or higher were not drawn.
- **Each query re-files and regathers state the game would not.** The test asks the query from extra
  probes and headings, so the broad-phase cache's reach and moved values drift from what the game's own
  run would have had. Each case starts from that drifted state, not from the save state's.
- **"Refused by a mesh" in the counts includes the crowded rings.** `NoSolid` answers both `solid` and
  `crowded` with no. The crowded rings are counted separately (272/248).
- **The interpreter's emulation of `put_back_refile`.** It is only run, not compared, because nothing
  outside it reads its result except the next query.

## Next

Finish: `./kit done S75`. The in-game query is S80.
