# 1 — keep race wins and fame (FINAL)

Task P354. 2026-11-16.

## What I did

- `camera_flashes.rs`: `Stadium::card_finishes` now takes a `Finish` (card, seat, event, track, Hall of Fame)
  instead of a bare card and lap count, so `Supercross::race_over` hears the human's finish with seat 0 and
  the profile's real Hall of Fame tables.
- `Finish::player` builds the first seat's card (PLAYER flag, keeps records, profile); `Finish::single` an AI
  rider's (no seat, track −1). `Finish::heard` is the call itself, used without a stadium too.
- `main.rs`: the finish is heard once. P334 had added a second `race_over` call for the human (seat 0, real
  tables) after the stadium's seat −1 call and copied its win/fame over; that second call is gone. Its answer
  goes into `race_watch.overs` as before, and `race_ended` / the results hand-over store the win in the
  seat's record and mark the Hall of Fame entries (both already in place from P330/P334).
- Side effect, now as the original: the human's winning card carries the PLAYER flag into the burst, so the
  flashes rise toward the track's player-win level, not the podium one.
- Nothing taken from ../opentennis (it has no race results).

## What the original does

`race_over` is heard once per card at the line, with that card's seat; P313/S192/P334's journals cover it.

## How it was proved

- Unit test `camera_flashes::tests::a_players_finish_keeps_its_win_and_records`: a player's win through the
  stadium counts the win (seat 0) and earns the sx best-lap and 3-lap entries from filled tables; the same
  card as an AI's earns neither. `race_over` itself was ported and tested in S192/P334.

## Not verified / not 1:1

- Only player 1's card is heard as a player's; a second human seat (split-screen race) is still heard as an
  AI's — the shell's race has no second-seat Hall of Fame or profile wiring yet (same scope as P334).
- The burst change for a human win (player-win level instead of podium) was not watched on screen: sx01's
  two levels are the same word (0x3e999999), so no sx track checked here shows a difference.
- No live race was ridden to a finish in the shell for this change; covered by the unit test only.

## Next

none
