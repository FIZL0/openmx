# 1 — effects leaf (PART)

Task F31b5n. 2026-11-09. Blocked on its split: F56, F58 (the effects items), F57, F59 (the emitters).

## What I did

- **Split the task.** The emitters are about 12 KB of VU0-heavy code once their callees are counted
  (the first kind alone: 3.2 KB plus eleven callees, among them a 2.9 KB second branch and a ray
  test; research/oracle.txt `# F31b5n`), and the effects reach 18 kinds of item virtual on flip-1.
  That is several sessions' work, so I ported the piece everything else hangs off (the effects
  listener's own work) and filed the rest:
  - F56: the effects items' turning virtuals (on `+0x18`, off `+0x1c`, key `+8`), with oracles;
  - F58: the effects items' steps (`+0x14`), one kind of which draws `rand`;
  - F57: the first particle emitter kind (11 listeners, all the emitter `rand` draws on C15);
  - F59: the second emitter kind.
  `plan add F31b5n …` is refused for a letter-ending id, so they are filed under `F`.
- **`game_sim::handling::effects`** (new): `step` is the effects listener's own work. It builds each
  table entry's flags byte, keeps a changed key and tells the bound items (`Out::Key`), turns each item
  on or off by its mask against its entry's flags and the table's bit (`Out::On` / `Out::Off`), and
  steps the items that are on (`Out::Step`). The closing walk is left to the caller, as `PartWalk` does
  for every leaf. Counts and the table pointer are read again where the original reads them again.
- **`PartWalk`** (tests/mxvatv_bike_ride.rs) runs `effects::step` for the effects leaf through a new
  `Effects` impl. Each item virtual is the original's, run through `PartWalk::original` (the leaf
  watcher, pulled out of `step`). That watcher asserts no bike or block word changes, books the
  instructions per kind (`the update's effects item <slot> <fn>`), counts `rand` draws, and asserts the
  item makes no closing walk of its own. `walk_served` now also asserts that the effects' own work never
  ran in the interpreter.
- **Oracle `the_effects_step_is_the_originals`**: 10 000 effects listeners generated in scratch RAM.
  Each has 0–6 entries (counts down to −1), tested words zero or not, keys kept or changed, the table's
  word zero or not, and 0–8 items (unbound, bound to an entry or to a stray flags byte; masks over all
  five flags; state bytes 0–3 and junk). Each runs through SLUS-21104's effects step, with the item
  virtuals hooked at fake addresses, and through the port. The hooks change RAM (state byte, counts) so
  the re-reads are tested. The item calls match in order and arguments, the original ends with its
  closing walk, and the scratch RAM matches. A mutant (stepping on `+8 & 1` instead of `== 1`) fails it.
- Nothing from opentennis: it has no particle effects or item tables ported.

## What the original does

research/oracle.txt `# F31b5n` has the clause-by-clause reading (from `./kit dis 0x004fa4a0 -p atv`).
The census below lists the item kinds flip-1 reaches. Only one item step kind draws `rand` (18 steps).
The emitters draw it 428 times.

## Census (flip-1, every port on)

| | instructions / calls |
|---|---|
| effects item calls (all kinds) | 4463 calls |
| item steps, six kinds | 19k, 52k, 101k, 2.7k, 333k (drew rand 18×), 1.2k |
| on / off / key kinds | 7 / 6 / 5 kinds, under 1k instructions each |
| emitter (11 listeners) | 1.82M, 428 steps drew rand |
| second emitter | 93k |

## How it was proved

- The oracle above (10 000 cases, bit-exact call log and RAM).
- `flip_replays_with_every_port_at_once` and `ride_replays_with_every_port_at_once` with the effects'
  own work in Rust: C15 word for word on every row, and the whole RAM matches the handing run's after
  every frame (flip-1 apart from the dropped cue's word, as before).
- `flip_needs_the_part_walks_effects` and `ride_replays_without_the_part_leaves` still pass. The
  "effects dropped" mode drops the port's own work, as it dropped the original's.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The task's acceptance is not met.** The effects items' virtuals and both emitter kinds still run
  in the interpreter inside `PartWalk`, so "the effects and emitters Rust's" is not true yet. Only the
  effects listener's own work is ported. Its reason is size (above); the rest is F56, F57, F58, F59,
  and F31b5n waits on them.
- **The effects port is held to the replays and the oracle only.** The oracle's items are stand-ins
  (fake virtuals with made-up reactions), not the real item kinds. The 18 real kinds are checked only
  on flip-1 and ride-2, where the RAM matches.
- **The PC exe was not read** for this function. The PS2 disassembly was short enough to read
  clause by clause, and the oracle is against the PS2 ELF.
- **The effects port is not the shipped game's yet.** Nothing outside tests calls
  `handling::effects::step`. The pure frame (F31b5) has to wire it in, with Rust items.
