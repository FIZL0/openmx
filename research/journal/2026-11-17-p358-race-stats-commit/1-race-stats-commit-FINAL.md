# 1 — race stats commit

Task P358. 2026-11-17.

## What I did

- `menu_results.rs`:
  - `RiderRecord` keeps six more words: play time in whole seconds (stat 0), feet ridden (1),
    preloads (2), clutch pulls (3), wrecks (4), seconds raced (5).
  - `RiderRecord::add_stat` is `profile_stat_add`'s cases 0–5.
  - `RaceTally` / `SeatTally`: what the commit reads (activity, player 0's profile slot, the score
    manager's clock, each player's feet, preloads, clutches, wrecks). `HandOver` carries one.
  - `Records::commit_race` is the commit's loop.
  - `records.txt` lines now hold nine words. A three-word line (before P358) still reads, the rest 0.
- `menu_race.rs`: `Results` keeps the tally. Restart and Quit's yes commit it before they leave
  (`Results::commits`), and the shell saves `records.txt`.
- `race_start.rs`: `hand_over_seats` builds the tally from the race's cards (seat 0 first).
- `main.rs`: a `race_clock` that adds dt every simulated tick from the gate's drop. It writes it into
  the handed-over tally each tick, so the commit reads it as the ride is left.
- `research/oracle.txt`: a P358 block (`results_stats_commit`, `results_score_commit`, every case of
  `profile_stat_add` and the stats page that names them).
- Nothing in `../opentennis` covers a profile's stats.
- Follow-ups: S386 (port the manager's clock and the tracker's odometer into game_sim), P364 (the
  score commit), P363 (a page that shows the records).

## What the original does

- `./kit fn 0x0029d1c0` (the commit): slot = `(app+0x3f4)[0] +0xcc`, the profile slot of player 0,
  used for every player. For k < event `+0x114`: the player's item from the scene's list
  (`vt+0x84(1, k)`), its MXUIInterface (`vt+0xa4`), the tracker through iface `+0x14` (card) `+4`
  (owner) `+0x24` (vehicle) `vt(+0x48)+0x40`. Event `+4 == 2` → stat 5 += manager clock. Then stat 0
  += clock, 1 += tracker `+0x78`, 2/3/4 += cvt.s.w of tracker `+0x38` / `+0x3c` / `+0x34`.
- iface `+0x18` is the rider's `+0xfa0` (`./kit fn 0x0027c220`), the score manager. Its `+0xf8` is
  the clock `score_manager_tick` adds dt to while running. Tracker `+0x78` is the odometer (the
  speedo reads it; S319: bike `+0x133c`, zeroed at the grid reset).
- `./kit dis 0x37ac40 0x37ae00` and the jump table at `0x004411d0`: 0 u16 `+0x114` += cvt.w.s;
  1 f32 `+0`; 2 u16 `+0x120`; 3 `+0x12c`; 4 `+0x138`; 5 f32 `+0x18`; 6 nothing; 7/8 laps; 9 races;
  10 wins. 6 slots each.
- The stats page `0x003b7030` names them: gameplay hours, miles/kilometres driven, preloads,
  clutches, wrecks, hours racing.
- Callers (`./kit xref 0x0029d1c0`): the results input's Restart (`0x0029cde0`) and the two dialog
  listeners (`0x0029e400`, `0x0029e600`). Each calls it only when no Hall of Fame mark waits. The Hall
  of Fame path never calls it, so a race that earned a record commits no stats. Ported that way.
- The commit also calls `0x002041a0` first: profile `+0xc0` += the card's banked total. Not ported
  (P364).

## How it was proved

- `tests/race_results.rs`:
  - `the_race_stats_match_the_profiles_stat_add`: 20,000 cases of the real `profile_stat_add`, stats
    0–6, every slot's nine words compared bit for bit.
  - `the_stats_commit_matches_the_results_menus_commit`: the real commit run whole over 10,000
    races. The registry lookups, the list, the items' interface slot and the vehicle's tracker slot
    are hooked; the score commit is stubbed. Every slot's nine words compared. A mutant that drops
    the single-race check fails it (case 8).
  - The records round trip, the old line format included.
  - I made each test panic once to check it ran and did not skip: 2,000+ changes per stat; 8,809
    commits, 6,988 single races.
- `tests/menu.rs` `the_results_buttons_commit_the_race_statistics`: Restart commits; Quit's no does
  not, its yes does; a waiting Hall of Fame mark and a league's Next do not.

## Not verified / not 1:1

- **The clock's value is the shell's, not the original's manager clock.** The game_sim race has no
  manager clock. The shell adds dt from the gate's drop to the ride's leave. When the original's
  manager stops (flag 8), and whether it runs on through the outro and the results menu, was not
  traced. No capture of the clock at the results menu was taken (no save state at a race's end
  exists). Filed as S386.
- **The feet are the race card's odometer (card `+0x20`), not the tracker's (`+0x78`).** The tracker's
  odometer is not ported. Its writer was not found. Both count distance ridden this race, but they
  are not the same word and were not compared. Filed as S386.
- **Only seat 0 has a profile, slot 0** (as P335). The original counts every player into player 0's
  slot. Ours does too, but split-screen seats have no profiles of their own yet.
- **The score commit (`0x002041a0`, profile `+0xc0`) is not ported** (P364).
- **No live check.** No race was ridden through to Restart in the shell to watch `records.txt` gain
  the stats. The commit is oracled and the menu flow is tested by pad reports.
- **Nothing shows the new stats yet** (P363).
