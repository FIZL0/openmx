# 1 — unlock shop

Task P369. 2026-11-18.

## What I did

- `menu_results.rs`:
  - `Shop` / `ShopCategory` / `ShopItem`: the store list, parsed from the disc's `appstart.res`
    `ui\store.dat` the way the original's store reader reads it (category `Type` → kind, kind 5 makes
    no items, `Price` default 100, 50 item slots a category).
  - `RiderRecord::unlocked` (a bitset, 16 categories × 50 items: the profile's unlock table),
    `owns`, `buy` (the shop's purchase: owned → `Owned`; points < price → `NotEnoughPoints`; else
    unlock and pay) and `unlock(item, pay)` (the profile's unlock, also the career's free one).
  - `records.txt` gains an `unlocked.N = cat:idx …` line after a record's `seat.N` line. Old files
    read with nothing owned; the line may come before the seat line.
- `menu.rs`: `Screen::Shop` ("Unlock Shop", word `shop` for `--screen`), routed through the race
  screens' rows and presses.
- `menu_race.rs`: `RaceMenu::shop`, `shop_tab`, `shop_said`; Triangle on the race-track pick opens the
  shop; Left/Right (L1/R1) step the category, Cross buys with seat 0's record (player 0's profile, as
  the original), Circle backs out. A "Not Enough Points" answer is a popup that the next Cross/Circle/
  Start puts away.
- `menu_screens.rs`: the shop screen (list of items with prices, owned ones dimmed and marked, the
  category heading, chips with the points as a plain number, the cost and the category count) and the
  popup; the race pick's hints gain "Unlock Shop".
- `main.rs`: reads the store list at start-up beside the Hall of Fame riders (`shop_list`).
- `research/oracle.txt`: `unlock_buy`, `profile_unlock` named, and a P369 block on the shop's purchase,
  the store reader and the career's free unlocks.
- `plan/TODO.md`: P369's own line names the two functions instead of their addresses (the audit flags
  named addresses), checkbox untouched.
- Nothing in `../opentennis` has a shop or unlocks.
- Follow-up P372 (later): what a bought item should do, since the port locks nothing.

## What the original does

- `./kit fn 0x0037d1b0` (unlock_buy): `item = cat[+0xc + 4*index]`; null → 0; `item+0x14 == 0` (not
  locked) or `points < item+0x10` (signed) → 0; else `+0x14 = 0`, `item+8 → +8 = 0`, returns the item.
- `./kit fn 0x0036df40` (profile_unlock): `profile+0xc4 → +0x54 + 200*item+0x1c + 4*item+0x18 = 1`,
  `+0x14 = 0`, link `+8 = 0`; with pay, `profile+0xc0 -= item+0x10`.
- `./kit fn 0x002f0da0` (the shop's pad handler): pad 0x36 (Cross) with shop `+0x1d4 == 0` and the
  lit item locked → unlock_buy with player 0's points; 0 → dialog "Not Enough Points" / "You do not
  have enough points to purchase this item.", sound 5; else profile_unlock(pay 1), `"%d"` of the
  points into the points text, sound 3. An owned item → sound 5 only. 0x32/0x33 step the category
  (wrapping), 0x34/0x35 the item (no wrap).
- `./kit fn 0x0037d320`, `0x0037ce30`, `0x0037bd80` and the five item ctors (`0x0037c020` …): the
  store reader (see oracle.txt's P369 block). Every ctor reads `Price` with default 100 and sets
  `+0x14 = 1`.
- `./kit fn 0x0036b900`, `0x0036afe0`: the career unlocks items with pay 0.

## How it was proved

- Function oracle, `tests/race_results.rs::the_shop_buy_matches_the_unlock_buy_and_profile_unlock`:
  10,000 purchases run through the original's `unlock_buy` then `profile_unlock` (pay 1), and ~1/8
  through `profile_unlock` with pay 0, against `RiderRecord::buy` / `unlock`. Points and prices at
  i32 edges, equal, ±1, negative and random; random owned tables; the item's locked word 1 or any odd
  word; linked record present or not. Checked: the answer, the points, all 800 table words, the
  item's locked word and the linked record's. A mutant (`<` → `<=` on the price) fails it.
- The store reader: `the_store_list_reads_as_the_store_reader_reads_it` on a made-up list (kinds,
  unknown kind, missing price). The real `store.dat` parsed in a throwaway test: 10 categories
  (Machines 5, Bike Classes 2, Supercross 20, Nationals 18, Freestyle 4, five activity lists of 11),
  prices as the file writes them.
- The screen: `tests/menu.rs::the_unlock_shop_spends_player_zeros_points` (open, step, buy, owned,
  not enough, back); a `--screen shop` shot on a private weston showed the disc's list and the points.
- The records round trip: `the_records_read_back_as_they_were_written`.

## Not verified / not 1:1

- Bought items unlock nothing in play: the port has no locks, so every track, machine, bike class and
  activity rides from the start. Filed P372 (later) for a person to decide.
- The career's free unlocks (pay 0) are not called anywhere: the port has no career.
- The shop's own pad handler (`0x002f0da0`) was not run in the oracle: its gate (the item's locked
  word, shop `+0x1d4`) is modelled from reading it. The `+0x1d4` busy flag is not ported (it is
  not known what sets it).
- Items here wrap with Up/Down (the menus' way); the original's item step stops at the ends with sound
  5. Sounds 3 and 5 are not played. The original's item pictures, posters and dialog look are not
  copied (menus are ours).
- The unlock table keeps 16 categories here; the original's has no limit we know of (the disc's list
  has 10). A store list over 50 items a category is cut at 50; the original writes past its slots.
- `Price` reads through the port's ini integer reader; the original's int reader (`0x0014b6f0`) was
  not oracled for this file (the disc's prices are plain decimals).
- Display names come from the store's section names (race tracks by the race pick's names), not from
  the `NameTextID` strings in `l_ui_eng.bin`.
- No live capture of a purchase in PCSX2: the oracle covers both functions whole.

## Next

Nothing; P372 when someone decides what owning an item means.
