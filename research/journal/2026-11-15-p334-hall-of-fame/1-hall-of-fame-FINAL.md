# 1 — hall of fame (FINAL)

Task P334. 2026-11-15.

## What I did

- New `crates/openmx/src/menu_fame.rs`: the Hall of Fame book (17 tables × tracks × 4 classes × 5 entries,
  same word layout as the game's 0x3ab4-byte block), the boot-time default generator, the insert, the
  list of tables a finished race earns, and a text save file `fame.txt` beside the records.
- The defaults are not stored: on first boot (no `fame.txt`) the shell generates them with the game's rand
  and the per-track base tables (copied as float bits from the RAM image), then saves them.
- Names for the defaults come from `rider\riders.dat` in appstart.res (second word of each line).
- Results screen: the placeholder Fame stage now shows "Congratulations, Player N!" with the tables the
  race earned, and opens the name keyboard (10 keys, prefilled with the last name entered or the rider's
  name). After the name goes in, a new Table stage shows each touched table with the new row in peach.
- `main.rs`: a finished race on an SX/Nationals track is now judged with a seated `race_over` against the
  book, so the results card can flag a Hall of Fame mark; the try (best lap, race time) is handed to the menu.
- `ps2-oracle`: the MMI op `paddub` (the insert uses it).

## What the original does

- `./kit fn 0x00375890` builds the defaults. For each SX track and class 0..3: start from the base value,
  then five times `x += 7 × max(unit(), 0.1)` with a name drawn after each value. Then the 3-, 5- and
  10-lap tables from `step(best[0] × laps)`. Then Nationals the same way. Then the five arenas with fixed
  score/count/time words, names drawn score 0–4, count 0–4, time 0–4.
- `./kit fn 0x00375680` draws a name: seed from the CD clock the first time (BCD sec + min·60 + hour·360 +
  day·86400), `srand`, then `rand() % riders`, take the second token of that riders.dat line, snprintf 30.
- `./kit fn 0x00378040` inserts: first slot the new value beats, shift entries and names down, write the
  value (the arena time table as an int) and strcpy the name.
- The results flag (`beats`) compares SX times on whole seconds, but the insert compares raw floats, so a
  flagged SX record can land lower than first, or not at all. Ported as is.
- Names and addresses are in research/oracle.txt (P334 block).

## How it was proved

- `tests/hall_of_fame.rs`, `the_defaults_match_the_original`: the generator run in the Ee interpreter
  over 40 seeds (rand/srand/clock/new/delete/snprintf hooked), every word, name, seed word and rng state equal.
- `insertion_matches_the_original`: the insert in Ee over 20 000 random tries, both "goes in" and "misses"
  hit more than 2000 times each, the whole block and names equal.
- `the_s196_image_is_made_again_from_its_seed`: the s196 RAM image's live Hall of Fame block is rebuilt
  from its seed word (first draw solved near the guess), all words and names equal; the riders list
  equals the context riders.dat.
- Plus the save file round trip and the tokenizer.

## Second attempt (2026-11-16)

- The first try (wip/P334) failed the merge gate in `tests/web.rs` and `tests/mxvatv_camera.rs`; the log
  was gone. Re-applied onto main; one conflict in `main.rs` (main's `card_finishes` now takes the podium
  burst), kept main's call and the P334 block after it.
- Cause found for web: `clock_now` read `std::time::SystemTime::now()`, which panics in the browser, and
  the shell calls it on a first boot with no `fame.txt`. It now reads `web-time` on wasm32, as
  `clock.rs` does. web, mxvatv_camera and hall_of_fame then passed (the browser tests ran, not skipped).
- mxvatv_camera: no cause found in this code (it mounts only vehicle.rs and chase.rs); it passed here, so
  most likely a load flake in the merge run.

## Not verified / not 1:1

- The first draw before the seed: the original's rand may have run before the defaults on boot; the port
  starts the generator from a fresh `Rng::new()` seeded by the clock. The s196 check had to solve the first
  draw, so fresh boots give different (but same-shaped) defaults than a given PS2 boot.
- The clock seed reads the system clock in UTC as BCD, not the PS2's CD clock (local time, set by the BIOS).
- Track counts are fixed at 24 SX, 22 Nationals, 5 arenas; the original reads them from the league lists.
- A NULL token (a riders.dat line with one word) is assumed to print as "(null)", as the EE's snprintf does in the oracle hook; not seen live.
- The tokenizer's overrun on a trailing double delimiter is not ported (riders.dat has none).
- Names longer than 29 bytes: the original strcpy overflows; the port cuts at 29. The keyboard allows 10.
- Freestyle (arena) tables 8–10 are generated and saved but no race result is ever entered in them.
- The name keyboard, the screens and their look are ours (menus are ours to design).
- The class of a try comes from the bike set's engine (`args.set`), not from a read of the game's slot.
- Only seat 0 (the one human) is tried; the two-seat order (who is asked first) follows hof_entry_page
  but is not exercised by a race.
- Wins now count in records too, a side effect of judging the finished race seated.
- No live capture of the original's name entry screen or of a record going in from a real race.

## Next

- Land it. Follow-up: freestyle Hall of Fame records once freestyle scores exist.
