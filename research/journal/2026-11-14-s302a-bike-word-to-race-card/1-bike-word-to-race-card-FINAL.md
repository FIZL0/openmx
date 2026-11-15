# 1 — bike word to race card (FINAL)

Task S302a. 2026-11-14.

## What I did

- `race::CardFeed` + `RaceCard::feed` (crates/game-sim/src/race.rs): the motocross bike's word to its
  score card. `Laps::tick` and `Race::tick` now take `&[CardFeed]` instead of `(position, look)` pairs
  (the gate watch reads `at`/`look` off it); `Laps::tick` feeds every card first, as the original's
  handler calls each card's owner first. `CardFeed::standing(at, look)` is the old pair's meaning
  (on the ground, height 0, still) for tests that walk riders.
- `Sim::race_frame` (lib.rs) takes the world and, when the race has laps, fills the feed from the
  rider: transform rows 3/2, `ride.chassis.vel`, `world.ground(position).y`, `ride.air.on_ground`
  (+0x574), `state.wreck.is_some()` (+0x748 != 0). The three callers pass `world` (lib.rs twice,
  rider_contact.rs once).
- research/oracle.txt: `score_card_feed` entry.
- Nothing taken from opentennis (no race cards there).

## What the original does

`./kit fn 0x0027dc80` (bike a0, card a1), reached through the bike's card-listener thunk
`./kit dis 0x00281b40` (this − 0xf9c, slot +8 of vtable 0x0045504c). The race handler
(`./kit fn 0x00262aa0`, the do-loop at the top) calls `card+4`'s vt+8 for every card, owner only.
With node = chassis(+0x7a4)+0x254: card +0x60 xyz = node +0xb0; +0x110 = chassis +0x260 (whole qword);
+0x108 = sqrt(mula/madda/madd of x, y, z); +0x120 xyz = node +0xa0 (look); +0x130 xyz = node +0x90 (up);
`ground_query(bike+0x7ac, copy of card +0x60 qword, 0, 0, 0)`, +0x104 = card +0x64 − point.y; bike
+0x574 ≠ 0 sets card 0x400 else clears it; bike +0x748 ≠ 0 clears 0x100000. Returns 1 (0 for a
null card). 0x003a0980 and 0x003e1d50 are the same copy for two other vehicle classes (frames at
+0x18c / +0x160, wheel word +0x530); not ported (no race rides those here).

The +0x108 word is the one the port called `LapCard::record_word` (copied into lap records, read as the
computer aim's speed): it is the bike's speed.

## How it was proved

- **Oracle**, tests/race.rs `the_bikes_word_to_its_card_matches_the_original`: 0x0027dc80 called
  alone on 20,000 generated bikes/cards (stray floats, NaN/Inf/denormal/−0, wheel word 0/1/any,
  wreck word 0/any), ground_query hooked to a generated point: flags, position, height, speed, look
  bit for bit against `RaceCard::feed`; the query's arguments (world, the card's new xyz with the card's
  own w, null normal/flags/record) checked; +0x110 / +0x130 checked to be the chassis velocity / up
  row. 10,018 ground-bit turns, 2,529 judge clears, 13,384 grounded. A forced failure confirmed it ran.
- **C31** (`the_race_handler_holds_c31s_live_sx01_laps`): every card now goes through `RaceCard::feed`
  off its captured position, look, velocity (+0x110) and ground bit before the handler; the speed the
  feed works out equals the captured +0x108 on every stepped row of both captures, and the whole
  handler still holds bit for bit.

## Not verified / not 1:1

- **The height on C31 / the live ground**: C31 captures only the cards, not the bike or the ground, and
  its start image has no score cards yet (the manager's list is empty), so the bikes' world pointer
  (+0x7ac) could not be found there to run the real ground query. The C31 test still takes the height
  from the captured card. In the live sim the port asks `World::ground` at the rider's position;
  that this is bit-for-bit the original's `ground_query` on sx01 (props, not only terrain) is not
  proved here.
- **The ground bit and look on C31** come from the captured card (C31 has no bike +0x574 or frame); the
  feed's copy of them is proved only by the oracle. That `Rider.ride.air.on_ground` is +0x574 and
  `state.wreck.is_some()` is +0x748 ≠ 0 rests on the existing oracle notes (bike_air_state, bike_wreck_entry).
- **The wreck clear on C31**: fed `wrecked: false` (no bike words); C31 shows no 0x100000 drop that
  would need it.
- **Velocity and up words (+0x110, +0x130)** are written by the original but not kept on the port's
  card: nothing ported reads them (listeners may), and adding fields would grow the snapshot wire.
- **The ground query's miss**: `World::ground` answers `NO_GROUND` (−1e6) where there is nothing; what
  the original's query leaves in the point then was not checked.
- The other vehicles' copies (0x003a0980, 0x003e1d50) are not ported.

## Next

-
