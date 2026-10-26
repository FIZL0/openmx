# 1 — freestyle event (FINAL)

Task S92. 2026-10-26.

## What I did

- `crates/game-sim/src/event.rs` (new): `Competition`, the freestyle event — the join, the 3.33 s
  countdown and its listener, the 120 s time limit, each card's clock and *out*, the manager's
  *all done* / *timed out* / end, the leader, the tie, the last-five-seconds warning, the
  target-reached latch, the verdict and the freeride total put back. `event::target(hit_dat)` reads the
  track's target.
- `crates/game-sim/src/score.rs`: two gates on `Score` — `shut` (the event's manager is not running or
  the card does not take part: no combo opens, and the card does not grow) and `out` (the card's time
  ran out: no combo opens, except the original's trick-start quirk below). Both default false, are
  walked into the hash only when true (so every freeride hash and platform vector is unchanged), and
  are on the snapshot wire.
- `research/oracle.txt`: an S92 section, including a correction of S79's card flag 0x1 ("networked")
  — it is *out of time*.
- Plan: P155 (the event in the shell), C9 (a live capture of an event).

## What the original does

**Which activity is the freestyle event.** The score manager's constructor (`./kit xref 0x25f080`) has
seven callers; the one whose activity reads `[HitData] StuntCompetitionScore` (key string at 0x444a40,
read by `./kit fn 0x3ddea0` into activity+0x1a0) is the *stunt competition*, constructor
`./kit fn 0x3dd340`. Its manager is embedded at activity+0x50 and its listener sub-object at +0x160.
Every activity offset below is manager offset + 0x50.

**Modes.** The manager's tick `./kit fn 0x25fcd0` dispatches on mgr+0x64 to six handlers. An objdump of
boot.elf scanned for `addiu $r, $zero, N` followed by a store to +0x64 (manager) or +0xb4 (activity)
found modes 1 (machine race), 2 (supercross, hit fast lap) and 3 (hit freeride, run, hit progression,
**stunt competition**) only. The three stores of 5 to a +0x64/+0xb4 found (0x36c008, 0x3881d4,
0x3d9968) are in unrelated objects (a GS packet builder and two UI objects). **Modes 4, 5 and 6 are
never set** — mode 5 (`./kit fn 0x265c70`, plus mode-5 branches in combo end `0x266fb0` and card
registration `0x260010`) is a "copy the leader's combo" game: a leader card at mgr+0xa4, strikes at
card+0x44, timed nodes (node flag 8) compared by held seconds within mgr+0xb4 (0.2), mgr+0xb0 strikes
to lose. It is dead code in MX Unleashed, so it was not ported (see below).

**The join** `./kit fn 0x3dd520` (human riders, player type 1): rider count +1, the player's
target-reached latch (+0xc0) and +0xcc to 0, the card re-registered through `0x260010(mgr, card,
0x80004, -1)`, which overwrites its flags word with 0x80804 (no combo, not taking part, not out), then
rider+0x734 = (int) the card's total and the total = 0.

**The start** `./kit fn 0x3de450`: every card's +0xc4 = 0, countdown length 3.33 (0x40551eb8),
interval 0.01 (0x3c23d70a), tie = 0, then `./kit fn 0x2603d0` (length > 0: flag 0x80, elapsed and
accumulator 0). It does not clear the running flag; a fresh activity's manager is not running.

**The countdown** (manager tick, flag 0x80, manager not running): if interval > 0 and a listener:
if interval ≤ accumulator or accumulator == 0 — subtract the interval if ≤, call the listener's vt+0x5c
with f13 = elapsed — then accumulator += dt. Then if elapsed < length, elapsed += dt; else flag 0x80
off, flag 1 (running) on, every card without 0x10 gets flag 2 (`./kit fn 0x266da0`), listeners' +0x78
and cards' owners' +0x68 called, and elapsed += dt. The mode handler does not run on that tick.

**The listener** `./kit fn 0x3de8c0` (thunk 0x3dec80 subtracts 0x160): HUD work at elapsed 0 and
≥ 1.0 over the activity's rider list; at elapsed **> 3.33** (the same float as the length) it sets
activity+0xf0 (mgr+0xa0, the time limit) = 120.0 (0x42f00000) and every card's +0xc4 = 120.0. At 30 Hz
the limit lands on tick 100, the tick the countdown ends; the clock counts from tick 101, and the card
is out at tick 3701.

**Mode 3** `./kit fn 0x268d00`: sets mgr flag 2 (all done) then clears it if any card has !1 && 2 &&
(0x8004) && !0x400000 — judged before the tick's clock, so a tick late. Per card taking part and not
out: distance odometer +0x20, jump statistics `./kit fn 0x267d90` if 0x80000 (the join sets it),
+0xb4 += dt; if mgr+0xa0 > 0: unless 0x40000 its +0xc4 -= dt; at ≤ 0 flag 1, mgr flag 0x10, +0xc4 = 0,
the out listeners (card owner +0x18, card chain, manager listener +0x14). Zones if mgr 0x200; rank by
total `./kit fn 0x2688b0` if 0x4000 (places at card+0x2c); end callbacks once when flag 2 and not 0x2000.

**What out means for scoring.** Combo start `./kit fn 0x266e00` opens only if the manager runs and is
not stopped (and mode ≠ 5 or card 0x100) and the card has 2 and not 1. The trick-start hook
`./kit fn 0x280b30` then, **if the card is out and the backflip watch (rider+0x1400) is 3, opens a combo
by hand** — so an out-of-time rider whose flip is round can still start one. Push needs only an open
combo; combo end ignores flag 1 (an open combo banks after time-out); grow (`0x267a90`) skips a card
without flag 2 entirely (no combo time, no head growth, no sum).

**The activity's frame** `./kit fn 0x3ddf70`, before the manager's tick (returns without it when the
player list is empty): over human players, the leader = whoever's total passes `(float) best` with
`best = (int) total` starting at -1 (so 100.7 passes 100.5's 100, and an equal 100.5 passes it too);
two riders: tie = totals equal; one: warning on unless time left is 0 or ≥ 5 or the target is reached;
one and target ≤ total and not latched: announcer `0x361730(..., 5, ...)`, latch. Two riders' warning:
on unless (leader and not tie) or left is 0 or ≥ 5 — **left read from the last player's card of the
first loop for both** (the original's stale pointer, kept).

**The end** `./kit fn 0x3dd870` (card out): not tie and ((target < total strictly, leader, alone) or
(leader, two riders)) → win message; tie → its own; else lose. Then the profile's points +=
saved + (int) total and the card's total += (float) saved.

**Targets on the disc** (`hit.dat` in `Tracks\freeNN_2.res`): free06 400000, free07 300000,
free09 100000, free10 150000, free11 200000.

## How it was proved

- **The clock**, `crates/game-sim/tests/event.rs` `the_event_clock_runs_like_the_original`: the
  original's manager tick (0x25fcd0) run in the interpreter on the first recording's RAM, on a manager
  of the test's making (the human's freeride manager copied into an unused block, its listener the
  stunt competition's own countdown listener 0x3dec80 reached through a fake vtable, every other
  listener a `jr ra`, the state's cards with their owners and listener chains cut and the join's
  flags), started by the original's 0x2603d0, beside `Competition::tick`. 6 runs (one card and two;
  30 Hz, 60 Hz and random dts from 0 to 0.5 s), each to ten ticks past the end, a card's clock held
  (0x40000) at random: **57 582 manager ticks bit-exact** — countdown elapsed, accumulator, limit,
  manager clock, running / counting / all done / timed out / ended flags, and each card's time left,
  seconds ridden, taking part and out (688 ticks counting down, 56 894 running, 9 cards out, 36 497
  with a clock held).
- **The gate**, `crates/game-sim/tests/score.rs` `the_event_gate_opens_like_the_original`: S79's card
  machine with the manager's running flag, the card's 2 and 1, and the flip watch thrown at random
  between trick starts, stops, grows (called only while the manager runs, as its tick does), one-wheel
  nodes, banks and wrecks: **10 000 operations bit-exact**, 864 trick starts refused a combo and 103
  opened one out of time on a round flip. It caught grow's flag-2 skip, which is ported.
- `the_countdown_at_thirty_hertz` (limit and run on one tick, out 3601 ticks later), `the_verdicts`
  and `the_target_from_hit_dat` by example; `crates/openmx/tests/event_target.rs` reads the five
  targets off the disc.
- S79's own card test still passes (its card now carries the gates, both false in the state).

## Not verified / not 1:1

- **Modes 4, 5 and 6 are not ported**, mode 5's timed-node matching included: no activity in the game
  sets them (the store scan above). If a scan missed an indirect store (a mode from a table or file),
  this is wrong; no such table was found and the only constructor callers set 1–3.
- **The activity's frame, the join and the end are not run against the original** — only read from the
  decompilation and tested by example. They walk the session's player list and the HUD and announcer
  objects (`DAT_0041f770` chains), which the interpreter's state does not have for an event. C9 would
  check them against a live event.
- **No live capture of a freestyle event**: the clock is proved in the interpreter on a fake manager
  only (C9).
- **The listener's HUD half** (elapsed 0 and ≥ 1: rider HUD elements, +0xcc on players) and the
  warning / announcer / result messages are not drawn: `Competition` only reports them (`heard`,
  `warning`, `reached`, `finish`'s verdict). The shell does not run the event at all yet (P155).
- **Not in the Sim**: `Competition` is not carried by `Sim`, so not hashed, snapshotted or replayed yet
  (P155); `Score`'s two gates are.
- **Jump statistics** (`0x267d90`, card +0xec..+0x100, set by the join's 0x80000) and the distance
  odometer (card +0x20) are not ported: they move no points.
- **Ranking** (`0x2688b0`, places at card+0x2c and the list reordered by total) is not ported; the
  verdict uses the activity's leader, not the places.
- **The card's clock hold (0x40000)** is ported as `Seat::paused`, but nothing in the game was found to
  set it; its writer is unknown.
- **The card flag 0x10** (skipped when the countdown ends) and the manager's *stopped* (0x8) are not
  modelled: nothing in the event was seen to set them.
- **The join's stale nodes**: the original's flag overwrite leaves the old node list on the card and
  never returns it to the shared pool; the port keeps the nodes (so a shut backflip adds the stale
  head's points to the combo's, as the original's hook does) but has no shared pool to leak from.
- **The original's card per player** comes from the rider; with two riders the tie is computed with
  the other player's card by player index. The port assumes seat order = player order.
- **The profile points** (`finish`'s second value) are returned, not stored anywhere.

## Next

Done; P155 makes it playable, C9 checks it live.
