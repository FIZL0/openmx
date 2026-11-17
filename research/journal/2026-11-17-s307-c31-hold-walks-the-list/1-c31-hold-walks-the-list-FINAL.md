# 1 — c31 hold walks the list (FINAL)

Task S307. 2026-11-17.

## What I did

- `tests/race.rs`, `c31_hold`: the order closure now walks the manager's list (head at `+0x34`, each
  card's next at `+0x1c8`), as `S13dRow::order` does. Before, it ranked the head and each card's next
  (in rider id order) by address, which is not the list's order.
- Card ids still come from address rank (the cards sit side by side in id order); only the order is
  walked. The walk stops at an address that is not a card, or after four cards.
- `C31Held` now keeps `long_way`, and the test asserts it: at most 1 on gas-held sx01, 13 on ai4.

## What the original does

The race handler runs over the cards in the manager's list order, so each card's spot search sees the
path scratch the cards before it left. The long-way-round distance hangs on that scratch.

## How it was proved

Same test, `the_race_handler_holds_c31s_live_sx01_laps`, both C31 captures, before and after:

| capture | long way round, address rank | long way round, list walk |
|---|---|---|
| sx01-gas.csv | 1 | 1 |
| sx01-ai4-by-rider.csv | 3,036 | 13 |

Every other count (steps, torn, laps, bests, finished, off-track) is the same. No word differs.

## Not verified / not 1:1

- The 13 ai4 ticks and the one gas tick (frame 293) still take the long way round: the AI riders' own
  path searches, which also leave scratch on the paths, are not ported. Not traced tick by tick here.
- Card ids by address rank assume the cards sit in id order in RAM (as the old code did); not
  re-checked against the image.

## Next

none
