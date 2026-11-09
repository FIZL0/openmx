# 1 — s13 results (PART, blocked on S13c)

Task S13d. 2026-11-09.

## What I did

I read the score manager's race-mode tick, where the results live, and then blocked the task on S13c.
I made no code change: the results can't be ported apart from the lap counting, for the reasons below.

## Why it waits for S13c

`score_manager_tick` (`./kit fn 0x0025fcd0`) switches on manager +0x64 once the countdown has ended (S13b
ported the countdown half). Mode 2, SingleRace, calls `0x00262aa0`. That one 5460-byte function holds
S13c's work and S13d's work together:

- **S13c's part:** each card's distance, the track-spline progress, the line and checkpoint tests
  (`start_gate_crossed` on lists +0x70 and +0x78), the lap step (`0x00260820`: card +0x24 += +0x28,
  +0x40 = 1), the off-track clock (+0xc4, flag 0x4000).
- **S13d's part,** using the same card fields. A card finishes when its flags +8 get bit 1:
  - +0xb4 (race time) = the first card's +0xb4 + this card's +0xbc;
  - +0x48 (lap time) = (+0xbc + +0x4c) / (total laps mgr+0x94 − laps done card+0x30);
  - +0x50 (best lap) = min(+0x50, +0x48), with 0 meaning unset, then clamped to +0xb4 / mgr+0x94;
  - +0x30 = mgr+0x94.

  While a card has not finished, +0xb4 and +0xb8 grow by dt, unless flag 0x10000 is set.

Porting the finish means fixing the score card's layout and its lap fields (+0x24/+0x28/+0x30/+0x4c/
+0xbc) in race.rs. That layout is exactly what S13c defines. Doing it in parallel would invent S13c's
model, and its merge would conflict with S13c's.

The brief also asks for "a captured sx01 race finish". That needs a whole sx01 race ridden to the flag
in PCSX2. With no pad here (uinput is root-only, see memory), it has to be ridden by a generated
`.p2m2` tape. The tape needs S13c's lap model to know where the line is, or a finish taken from an
S196-style save state near the end of a race (with laps cut to 1 in the event, +0x115).

## What the original does at the race's end (read, not proved)

From `context/notes/fn_boot_00262aa0.c`, after the card loop:

1. **Waiting for the others.** Mode != 3 (Ghost), manager flag 4 is up (set when a finished card has
   +8 bit 4 or bit 0x8000, i.e. a human finished), flag 2 is clear, and mgr+0x9c > 0:
   - while mgr+0x98 < +0x9c, +0x98 += dt;
   - otherwise flag 2 goes up and every card gets bit 1 (forced to finish).

   At the top of the function, flag 2 is set and then cleared again if any card is still racing (bit 2
   set, bits 0x4001 clear, bit 0x8004 set, bit 0x400000 clear).
2. **Race over, once** (flag 2 up and 0x2000 clear → set 0x2000). The function walks the card list.
   Whenever the next card's +0xb4 is less than this card's, the next card's time becomes this card's
   time + `rand()·2⁻³¹`. So the times are made to rise down the list, and riders who did not finish
   get invented times, one draw per fix. Then come each entity's vt+0x1c and its followers' (+0x1c4
   chain), and the listener's vt+0x18 with its children. The listener call is most likely what tells
   the front end and starts the fireworks: the strings `FireworksAnimator%02d` / `Fireworks%02d` /
   `FIREWORKS` are in the ELF, but their binder was not traced.
3. **Hand-back.** In non-Ghost modes, when flag 0x4000 is up, it calls `0x00261b80` (1084 bytes, only
   this caller); if that returns 0 the tick returns 0. Without flag 0x10 it calls `0x002628f0`. Ghost
   uses `0x002629d0`. The tick's 0/1 return is not read here.
4. `0x00266d50` (68 bytes), called from `score_manager_tick` after every mode except 5, is probably
   the places (card +0x2c) update. Not read.

## Not verified / not 1:1

- **Nothing is ported.** Places, finish times, best lap, the finish call and the hand-back are all
  open. They need S13c's score card and lap model first. Blocked with `--after S13c`.
- **Everything in "What the original does" was read from the decompilation only.** None of it is
  oracled or captured yet. Ghidra can drop stacked arguments (see memory), so check the vt calls'
  arguments with `./kit dis` before porting.
- **The fireworks binder, `0x00261b80`, `0x002628f0` and `0x00266d50`** have not been read.
- **No sx01 race finish was captured.** No pad here, and no race-finish state exists in any slot.

## Next

When S13c lands, port the finish stanza, the race-over block and the places update onto S13c's card.
Oracle them per function against the interpreter, then capture a finish.
