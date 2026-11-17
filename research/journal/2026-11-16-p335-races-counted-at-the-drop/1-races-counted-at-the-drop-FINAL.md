# 1 — races counted at the drop (FINAL)

Task P335. 2026-11-16.

## What I did

- Traced who adds the profile's races stat. It is not a stats commit at the race's end: the only
  stat-9 call to `profile_stat_add` in the ELF is in `race_drop`, the gate's drop.
- Ported that loop as `Records::count_race` (`crates/openmx/src/menu_results.rs`).
- `Menu::race_gate_dropped` (`menu_race.rs`) counts it in the records while a menu race is under way
  and marks them changed, so the shell saves `records.txt`.
- The shell (`main.rs`, the race block beside the lap line) calls it when seat 0's race gate goes from
  closed to open, for seat 0 with event mode 0, activity 2 — the same seat and event the P334 win
  path uses.
- `research/oracle.txt`: `race_drop_races` / `race_drop_races_end`, the loop as a stretch.
- Nothing in `../opentennis` covers this (a race-count stat of this game's profile).

## What the original does

- How it was found: `./kit xref 0x0037abf0` gives 73 calls; reading `$5` (the stat id) before each
  call shows one `addiu $5, $zero, 9` (0x002fc0a0, in `race_drop`) and one `10` (`race_over_card`).
- `./kit fn 0x002fbdd0` / `./kit dis 0x2fc040 0x2fc0d0`: after the overlays, `+0x4c0 = 1.0` and the
  gate's open bit, if front (`app+0x3d4`) `+4 == 0` and its event (`+0xc`) `+4 == 2`, then for
  `k < event+0x114` (a byte, re-read each pass) with `app+0x3f4[k] != 0`:
  `profile_stat_add(1.0, app+0x3e4, 9, profile+0xcc)`.
- So a race counts when the gate drops, win or not, and a race quit before the finish still counts.
  The win (stat 10) counts at the finish, under the same mode/activity condition.

## How it was proved

- Function oracle, `crates/openmx/tests/race_results.rs` `the_race_count_matches_the_gate_drops`:
  20,000 fronts run through the original's loop as a stretch (with the real `profile_stat_add`)
  against `Records::count_race`. Modes −1..1, activities 0..4, 0–12 players and random bytes up to 255,
  seats without a profile, several seats on one slot. Every slot's races, wins and rate compared bit
  for bit. It fails if the mode check is dropped (checked by breaking it once).

## Not verified / not 1:1

- **Not run end to end in the shell.** No menu race was ridden to see `records.txt` gain a race at the
  drop: that needs a `--menu` run on a private weston and menu navigation. The shell's part is a
  closed→open check on seat 0's gate flag plus one call; the count itself is oracled.
- **Only seat 0 is counted**, with a fixed event (mode 0, activity 2), as P334 does for wins. The
  original loops every player seat with a profile; the shell has no profile per split-screen seat yet.
- **Records are kept per seat, not per profile slot** (`profile+0xcc`), as P330 chose for wins.
- **A restart re-counts only if the gate closes again**: the shell counts each closed→open edge, as
  the original counts each drop; a restart path that re-opens the race without closing the gate was
  not looked for.

## Next

None.
