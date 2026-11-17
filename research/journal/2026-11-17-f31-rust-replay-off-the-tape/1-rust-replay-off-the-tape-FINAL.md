# 1 — Rust replay off the tape (FINAL)

Task F31. 2026-11-17.

## What I did

- Checked what F31's brief still lacked after F31a/b/c and F31b's follow-ups (F31b5a..v, F67):
  - "C15's ride replays bit-exact, flip through its wreck and put-back" in crates/game-sim tests:
    `ride_replays_in_pure_rust` and `flip_replays_in_pure_rust` already did it, with the interpreter
    running 0 instructions (F31b5's journal, 11-17).
  - "feed the tape's .ticks.csv": not done. The replay poked the reader's words (`READER`) from each
    row. Only gas, brake, the stick and R1 were tied to the tape (`ticks_land_one_row_later`).
- `tests/mxvatv_bike_ride.rs`:
  - `tape_words` / `feed_tape`: the reader's 14 words built from a tape tick and the bike's own wreck
    state. Rules read off the two captures (oracle.txt `# F31`):
    - +0x310 brake = square; +0x314 gas = cross, 0 while wrecked;
    - +0x12a8/+0x12ac the left stick; +0x14c8 the stick's way (up 1, down 2, left 3, right 4);
    - +0x14ec trick byte: left 0x10, right 0x20, up 0x40, down 0x80, R1 4 while not wrecked; its upper
      three bytes kept from the image;
    - +0x14d4 = 1 while riding, 0 while wrecked (the put-back request);
    - d-pad +0x12a0/+0x12a4, spin hold +0x12b4, clutch/reverse +0x12c0/+0x12c4, +0x17ac: 0;
    - the flip hold +0x12b0 still comes from the row (see below).
  - `the_tape_gives_every_reader_word`: on every row past the first of both captures, `tape_words`
    equals the row's reader words, word for word, and covers every word `READER` pokes.
  - `replay_pure` takes `"tape"`: row 0 checks the image already holds row 0's reader words (row 0
    reads the tick before the tape), then row r is fed tick r − 1. The wreck state is the second run's.
  - `replay_in_pure_rust` (so `ride_replays_in_pure_rust` and `flip_replays_in_pure_rust`) now runs
    with `"tape"`. The row-fed version is not kept: past row 0 the two feeds are the same words.
- Filed F31d (pad reader in Rust, flip hold off the tape), F31e (the put-back spot's writer) and F31f
  (`later`: run the port in the app's `Handling::MxVsAtv` ride).
- No code under `src/` changed. Took nothing from `../opentennis`: it has no MX vs. ATV bike.

## What the original does

- The pad reader (`./kit dis 0x004e36c0 0x004e3c70 --program atv`) copies the flip hold +0x12b0 straight
  from the pad record (entries + 0x31c, entry 0x21's value) and the spin hold +0x12b4 from entries
  + 0x304. On flip-1 the flip hold reads 992.97 (0x44783e10) on rows 66..72, while the stick is down.
  MD164 saw −992.97 in Untamed's pad word too; neither is explained.
- The put-back spot: +0xd38 is not part of it. It is the vehicle sphere gather's squared distance from
  its last gather point (`# MD83b`; it climbs from 9.6 and +0xd3c = 100 is its limit). The spot proper
  is +0xd40..+0xd6c: a point (w 1), a flat heading (y 0, w 1) and the terrain normal. Its words are no
  copy of the body, node, record or wheel blocks of the same or the row before. No writer found: the
  overlay has no direct store at +0xd40/+0xd50/+0xd60, only many `addiu +0xd40` pointer builds (F31e).

## How it was proved

- `the_tape_gives_every_reader_word` and `ticks_land_one_row_later`: pass (`./kit check -- -p
  game-sim --test mxvatv_bike_ride -- the_tape_gives_every_reader_word ticks_land_one_row_later`).
- `ride_replays_in_pure_rust` and `flip_replays_in_pure_rust` fed off the tapes: pass. Every row
  word for word with C15 (body, node, record, wheels, engine), ride-2 never wrecks, flip-1 wrecks on
  row 125 and is put back on row 193, the interpreter runs 0 instructions.
- Not a skip: a temporary panic on the last row's tape feed made both fail with "fed 458 rows off the
  tape" (ride-2) and "fed 464 rows" (flip-1), so every earlier row had matched on the tape's words.
  Put back (file compared byte for byte with the kept copy).
- Original rides keep every replay hash: no shipped code changed; the gate (`./kit check --all` inside
  `./kit done`).

## Not verified / not 1:1

- **The flip hold +0x12b0 is still the row's.** The reader copies it from the pad record's entry
  0x21, whose 992.97 is unexplained. Porting the pad record is F31d.
- **The put-back spot +0xd38..+0xd6c is still the row's**, row r's into frame r + 1. Its writer runs
  outside the sim frame and was not found (F31e).
- **The reader's rules are read off two tapes, not the reader's code.** C15's tapes press only cross,
  square and R1, hold the stick at −1, 0 or 1, and never touch the right stick, d-pad or clutch
  (`tape_words` asserts it). Stick thresholds, analogue gas, and whether the stick's way bits are kept
  while wrecked are not shown. The real reader is F31d.
- **Not the app's ride.** The pure-Rust frame runs over state 4's RAM image held by an `Ee`. The app's
  `Handling::MxVsAtv` ride is still MX Unleashed's `Ride` with `handling::step`'s clauses. Moving the
  ports there needs the whole world in the original's layout from the disc, or the ports on Rust
  structures: big, filed as F31f (`later`).
- **Only C15 ride-2 and flip-1**, and the earlier leftovers in F31b5's entry
  (`../2026-11-17-f31b5-whole-frame-closed/`) still stand. MD63f's wreck gravity never runs on them
  (F31b's entry).

## Next

Nothing; `./kit done` closes F31. F31d and F31e take the last row words out of the replay.
