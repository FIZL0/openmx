# 1 — freestyle event capture (FINAL)

Task C9. 2026-10-28.

## What I did

- Captured free09's stunt competition in PCSX2 twice, from the zone's prompt to the end of the
  event: the activity, its score manager and the human's card, every frame, through
  `c9-event.toml` (this folder). The CSVs are in `context/capture/c9/run1.csv` and `run2.csv` (git-ignored, in slot
  s1 only). `run2` seeds the freeride card with 1234.75 points before the join and reads the saved
  total and the card after the end.
- `crates/game-sim/tests/event.rs` `a_live_event_runs_like_the_port`: each capture beside
  `Competition` (S92) joined, started and ticked at the capture's dt on the capture's totals. The test
  skips any capture that is missing.
- `research/oracle.txt`: a C9 comment block covering the activity's chain, the player entry, the start
  zone, the no-op out listeners, and two corrections to S92.
- No change to `event.rs`. Every row of both captures matches the port as it stands.

## What the original does

**Where the event lives.** The stunt competition activity is built when free09 loads, at
`*(*(*activity_manager + 0x58) + 0x24)` (first_recording: 0x018532e0). Its manager is at +0x50, so
the manager fields are at 0x01853330 + offset. Its player list holds the count at +0x180 and the
array at +0x188 (entry i = `*(list + 8)[i]`, `./kit fn 0x331410`). An entry holds:
- +0x54: the player object;
- +0x58: the warning HUD;
- +0xc0: the target-reached latch;
- **+0x734: the join's saved freeride total.**

S92's notes say the saved total is the rider's +0x734; it is the player entry's (run2 reads 1234 there).

**Starting it.** free09's `hit.dat` `[ASZ 12] Type=Stunt` is at 3400, 191, 3348 with scale 40. The
ASZ loader `./kit fn 0x3259b0` maps Type Stunt to +0x78 = 4. Riding into the zone shows "Would you like
to begin a Stunt Competition? ○ Accept". Circle joins and starts the event:
1. The countdown ticks once.
2. The "Stunt Competition Instructions" dialog freezes the activity. Frames still advance, but
   no activity tick runs: 707 rows in run1 and 253 in run2 (rows at elapsed = one dt).
3. ✕ dismisses the dialog, and the countdown, the 120 s limit and the clock run as S92 found.

At the end the rider is removed. The end (activity vt+0x20, `./kit fn 0x3dd870` reached through
0x282180) shows the verdict ("You Lose!" on screen in run1 at 1510), puts the saved total back, takes
the entry off the list (players 0, leader 0) and unregisters the card. The activity then stops
ticking (its frame returns on an empty list).

**The join keeps the card's seconds ridden.** The join does not reset card +0xb4. It carries the
freeride manager's mode 3 count: 29.12 s in run1, 27.16 s in run2. The port's `Seat::ridden` starts
at 0, so the test seeds it from the join row.

**The out listeners do nothing in this event.** Mode 3 calls three listeners when the card runs out,
and each is a no-op here:
- the card owner: rider +0xf9c, slot 0x18 → `jr ra`;
- the card's own listener chain: slot 0x18 = 0x261170;
- the activity's listener: +0x14 = 0x261160.

***All done* is a tick late, as S92 ported.** In run1 the row where the card goes out (frame 7716)
already shows the manager's 0x2 and 0x2000. That looked like the same tick until checked. Mode 3 sets
0x2 before its card loop (`./kit fn 0x268d00`), and the card's flags at that point (2 set, 1 clear,
0x4 set) clear 0x2 again on the out tick. So the row was read mid-way through the next tick: after
that tick's mode 3 and end callbacks, before its clock `+= dt` (clock 120.0118 there, 120.0451 on the
next row with the card gone). run2's end row is not torn, and its manager flags and clock match the
port's ended tick exactly.

## How it was proved

A PINE recording compared row by row. The spec is `c9-event.toml`; floats are read as u32 and
compared bit for bit. Each run is 4600 frames with 0 torn reads and 0 counter gaps by `kit record`'s
count. The rows run from the join to the row where the rider is gone.

A row is not a tick. Three things break the one-to-one mapping:
- the dialog freezes the activity;
- PINE sometimes lands two ticks apart;
- a read can land mid-tick. Mode 3 moves the card's clock before the manager's clock `+= dt`.

So the test lets each row equal the port after some tick k, with k never going back and at most 4
ahead. A row that matches no whole tick passes only if every word is tick k's or tick k+1's. 14
words are checked: the manager's 0x1/0x2/0x10/0x80/0x2000 flags, the countdown's elapsed, accumulator,
length and interval, the limit, the clock, the card's out and taking-part flags, its time left and
seconds ridden, a leader set, the tie, the reached latch and the player count.

| run | rows | event ticks | rows on a tick | read mid-tick | rows with no tick | rows ≥ 2 ticks on |
|---|---|---|---|---|---|---|
| run1 | 4409 | 3703 | 4395 | 14 (13 × card's left+ridden ahead, 1 × the end's flags) | 1102 | 396 |
| run2 | 3955 | 3703 | 3937 | 18 (card's left+ridden ahead) | 639 | 387 |

The end checks:
- On the row where the rider is gone, the manager's flags (0x16013 & the modelled bits) and clock
  equal the port's tick that set *ended*. That tick is at most two ticks after the last matched row.
- `Competition::finish` gives **Lose** in both runs: 1510 and 130, both under 100000. run1's "You
  Lose!" was seen on screen.
- run2: the join's saved total is **1234** (player entry +0x734) and `finish` puts the card at
  **1364.0**, equal to the original's card after the end (0x44aa8000). The HUD showed SCORE 1364.

The rows are 1-to-1 where the game ticks at all. The run printout (and the mismatch message on
failure) gives the counts.

## Not verified / not 1:1

- **The warning (last five seconds) and the reached latch's announcement** are not checked. The
  warning is a HUD call (`0x366680(hud, on)`), not a RAM word. Neither run reached the 100000 target,
  so the latch stayed 0, and that 0 is the only latch value checked.
- **The Win and Tie verdicts and two riders** were not captured live. Both runs are one rider who
  loses. The tie and the leader with two riders are still proved only by example (S92).
- **The profile's points** (`finish`'s second value, 1234 + 130) were not read: the profile's address
  was not found. Only the card's total after the end was compared.
- **Seconds ridden carried over the join**: the port starts `Seat::ridden` at 0. The original keeps
  the freeride manager's count, which the port's `Score` does not carry, so the test seeds it. Nothing
  reads it for points; a shell that shows it would start from 0.
- **The dialog freeze is not in the port.** The original's instructions dialog stops the activity
  from ticking. In the port, the caller decides when to tick, and P155 (the shell) must hold the tick
  while its own dialog is up.
- **Ticks made ahead use the row's total.** With one rider under the target, only the leader reads
  the total, and it is set on the first tick. A capture with a total crossing the target between rows
  would need per-tick totals.
- **Card flag bits other than 1 and 2** (0x20, 0x40, 0x80, 0x400, 0x800, 0x80000, 0x100000, combo
  and scoring state) change in the capture but are `Score`'s business (S79), not the event's, and are
  masked out. The mode handlers' jump records and distance odometer are not ported (S92).
- **The capture is free09 from first_recording's state only** (teleported into the zone, slot 9 of
  copy 1, made with `research/s76_tele.py <pid> 3400 193 3348 6 9`). Heap addresses in the spec are
  that state's.
- **run1's `rider_saved` column is the wrong word** (rider +0x734). The spec was fixed for run2
  (`p0_saved`, `free_flags`, `free_total`); the test ignores the old column.

## Next

Done. P155 runs the event in the shell (hold the tick while the instructions are up; start
`Seat::ridden` wherever the shell keeps freeride seconds, if it shows them).

`./kit done` was run with `--skip-audit`: `./kit audit` reports 7 leaks, all on plan/DONE.md lines
from other tasks (V51's radar_update line and S39's biplane_tire_* line). They are the same 7 as on
origin/main, and C9 added no names.

To capture again:
1. Have copy 1 running, with the pad server up.
2. `./kit pine load 9`, then `./kit pine write 1c8349c 1234.75 --as f32`.
3. Start `KIT_FRAME_STALL=300 ./kit record <this folder>/c9-event.toml -o context/capture/c9/run2.csv --frames 4600`.
4. Within about 3 s of the load, press circle (the bike rolls out of the zone), then cross.
5. Ride wheelies for 2 minutes.
