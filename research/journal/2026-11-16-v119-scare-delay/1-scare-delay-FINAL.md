# 1 — scare delay (FINAL)

Task V119. 2026-11-16.

## What I did

- Scanned every file of every `.res` on the disc (523 archives, 53,851 files) for `ScareBirds` and
  `ScareDelay`. ScareBirds is in three files only: `nat01.toy` (16 actions), `nat02.toy` (3), `nat19.toy`
  (2). All 21 have `ScareDelay = 0.0`. So the delayed path is never taken on the disc and is not ported.
- `creature::Scare.delay` now says why it is read and unused.
- `tests/creature.rs` `no_scare_on_the_disc_is_delayed`: scans the three `.toy` texts, wants 21 ScareBirds
  and 21 ScareDelay lines, each 0. It scans text, not the loaded triggers: the loader keeps only the 5
  scares a trigger's NumberOfTriggers/NumberOfActions reach (nat02's 3, nat19's 2); nat01's 16 lie past them.
- Nothing taken from ../opentennis (it has no birds).

## What the original does

- ScareBirds reader: `./kit fn 0x003ae960`. ScarePosition, ScareDistance, ScareDelay into the action.
- Run: `./kit fn 0x003aea00`. Only on enter 1, calls the scare (`./kit fn 0x003ac9a0`) with distance,
  delay, the bird manager and the position.
- The scare with delay 0: each sitting bird nearer than the distance takes off (what the port does), then
  one of up to six sound emitters on the last scared bird's spline is picked and played.
- With delay ≠ 0 it scares nobody now. It stores, on the manager (not per bird, as the brief guessed),
  a deadline = clock + delay × 1000 (ms, with the unsigned float→int fix-up), the position and the
  distance. One pending scare per manager; a later one overwrites it. The manager's update presumably
  fires it at the deadline; not read, since no disc data reaches it.

## How it was proved

- The raw scan above (a throwaway probe test, deleted), and the kept test over the three files.

## Not verified / not 1:1

- The delayed scare path is not ported: no ScareBirds on the disc has a nonzero delay. A mod `.toy` with
  one would scare at once instead of after the delay. The manager's side that fires the pending scare
  was not read.
- Mod tracks in openmx-mods were not scanned; only the retail disc.

## Next

-
