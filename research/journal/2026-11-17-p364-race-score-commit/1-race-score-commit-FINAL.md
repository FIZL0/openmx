# 1 — race score commit

Task P364. 2026-11-17.

## What I did

- `menu_results.rs`:
  - `RiderRecord::points` (profile `+0xc0`) and `add_points`: `cvt.w.s` of the bank, a wrapping add.
  - `SeatTally` gains each player's own profile slot and their card's banked total.
  - `Records::commit_score` is the score commit. `commit_race` runs it first, as the original does,
    even when player 0 has no profile.
  - `records.txt` lines hold ten words now. Nine-word (P358) and three-word lines still read, points 0.
- `race_start.rs`: seat 0's tally has profile slot 0 (the only profile, as P335/P358).
- `main.rs`: each tick from the drop, and at the Over, each seat's `rider.score.total` goes into its
  tally beside the clock, so the commit reads the bank as the ride is left.
- `menu_race.rs`: a doc line only. `tests/menu.rs`: one initializer gains `..default()`.
- `research/oracle.txt`: the commit marked ported; a P364 block on who reads `+0xc0` and why a race
  card banks.
- Nothing in `../opentennis` has profile points.
- Follow-ups: P369 (the unlock shop that spends the points), C42 (a capture of a race trick's bank).

## What the original does

- `./kit fn 0x002041a0`: the RaceStatusVehicleInterface id; for k < event `+0x114`, when
  `(app+0x3f4)[k]` (player k's own profile) is set: item `vt+0x84(1, k)`, iface `vt+0xa4(id)`, card
  `*(iface - 0x24)`; profile `+0xc0 += identity(profile, cvt.w.s(card +0xdc))` (`./kit fn 0x0036ea50`
  returns its second argument). Called from the stats commit (`./kit fn 0x0029d1c0`) before anything,
  with no profile check of its own beyond the null test.
- Who reads `+0xc0`: I objdumped the ELF and listed every function with a `+0xc0` load or store in the
  profile and front-end ranges, then read them:
  - the unlock shop's purchase (`./kit fn 0x002f0da0`): `./kit fn 0x0037d1b0` unlocks an item when its
    cost `+0x10` <= player 0's points, else "Not Enough Points"; `./kit fn 0x0036df40` takes the cost
    off; the shop prints the points.
  - the profile save / load (`0x0036e440` / `0x0036e710`) copy it to save data `+0x5718`; the profile
    reset `0x0036e1f0` zeroes it.
- Does a race card bank? `./kit fn 0x00266e00` (combo start) needs the manager running and the card
  taking part; only event mode 5 asks more. `./kit fn 0x00266fb0` (combo end) banks in every mode. So a
  trick landed in a race banks into the card's `+0xdc`, and the score commit pays it out.

## How it was proved

- `tests/race_results.rs` `the_score_commit_matches_the_results_menus_commit`: the real commit, run
  whole, over 10,000 races. The interface lookup, the player list and the items' interface slot are
  hooked; the identity runs as is. 0–12 players, each with a profile slot or none, slots shared, banks
  of any float (fractions, negatives, past 2^31, NaN) and points near both ends of the word. Every
  profile's points compared.
  - I made it panic once: 8,555 races scored, 27,768 points words moved, 1,276 saturating banks.
  - A mutant using Rust's `as i32` instead of the EE's `cvt.w.s` fails it.
- The records round trip covers the points and the nine-word line.
- `s282c-sx01-race.ee.bin`: the four race cards' `+0xdc` read 0 at 10.57 s (no trick yet).

## Not verified / not 1:1

- **No race card was seen with a non-zero bank.** That a race banks is from reading combo start and
  end; the one race image has no trick in it. Filed as C42.
- **The bank fed in is the port's `rider.score.total`, which P243's landing lockout can hold back**
  (the user's own addition). With a lockout still up when the ride is left, the port's bank can lag
  the original's by that last landing.
- **Only seat 0 has a profile.** Split-screen seats have none yet, so their banks are not paid out;
  the original pays each player's own profile.
- **Nothing spends or shows the points.** The unlock shop is not ported (P369).
- **No live check.** No race was ridden to Restart in the shell to watch `records.txt` gain points.

## Next

Done.
