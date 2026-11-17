# 1 — race over every seat (FINAL)

Task P350. 2026-11-16.

## What I did

- A race with more than one local seat is now one race over all of them. It still lives in seat 0's
  Sim. The other seats are each their own Sim, so they are told to it as guests.
- game-sim:
  - `race::Race` has `guests` (one `CardFeed` per rider outside its Sim) and `guests_did` (what the
    last tick did to them). Neither is walked or on the wire (`wire! … skip`). Both are empty unless a
    shell fills them, so one-Sim races tick as before.
  - `race::CardFeed::of(rider, world, laps)` is what `Sim::race_frame` built inline, split out so the
    shell builds a guest's feed the same way.
  - `Sim::race_frame` ticks the race over its players, then the guests. It only applies catches to its
    own riders and leaves the guests' catches, the start step and the drop in `guests_did`.
- main.rs:
  - The grid is laid out for at least as many riders as there are local seats. Seat `i` stands in grid
    card `i`'s lane, facing the grid. The AI riders take the lanes after the seats (`race_ai_lanes`),
    whatever the seat count, not just with one seat.
  - Race player order: seat 0's Sim (its human, then its AI riders), then seats 1.. (`race_player`).
    Lap cards go in that order: the other humans get `HUMAN_CARD`, the AI riders `COMPUTER_CARD`.
  - The roster is drawn with `humans` = the seat count (`race_start::spawn_ai` takes it). It is handed
    to seat 0's Sim with `humans: 1`, so there the AI riders are players 1...
  - In the riders' walk, just before seat 0's pass, `race_guests_in` feeds the other seats' riders to
    the race and counts each bike that rode this pass on its card's tracker (`Laps::bike`, clutch off
    the seat's controls). Just after, `race_guests_out` does to their bikes what the Sim does to its
    own: a catch pushes the bike back and sets the gate penalty, then the start step, then the drop's
    release.
  - Every seat's HUD reads its own card off seat 0's laps. The results name each seat's human
    (`race_start::hand_over_seats`; `hand_over` is the one-seat case).
  - New lines: `race: player N in lane L place P; grid …` and `race: player N at … at the drop`. The
    `N cards, M computer` line counts cards with no player bit.
- Test `a_two_seat_race_has_one_grid_and_one_lap_line` (crates/openmx/tests/race_menu.rs):
  `--players 2 --racers 4` on sx01 gives the second seat its own lane behind the gate, two AI riders,
  four cards with two computer ones, the second seat held at the drop, and its card (race player 4)
  ranked.
- Took nothing from opentennis: it has no races or shared race state across seats.

## What the original does

- Not read for this task. The original runs every human and AI rider in one game world, so one race
  covers them all. That is what this rebuilds across the shell's per-seat Sims.

## How it was proved

- This is wiring, so it was checked by riding the shell, not by an oracle:
  - `./kit check -- -p openmx --test race_menu`: 4 passed, the new test and P313a's one-seat test
    included. The one-seat lines are unchanged.
  - By hand, `--players 2 --racers 4 --seconds 8`: both humans sit at z 447.11 at the drop (the gate is
    at 454.58), the two AI riders are in lanes 3 and 0, and the lap line ranks all four players.
  - `./kit check -- -p game-sim --lib --test hash --test race --test snapshot`: all passed. The Sim
    size pin is unchanged because the race is boxed.

## Not verified / not 1:1

- **The AI riders and the other seats' riders do not meet.** The AI riders are players in seat 0's
  Sim. Between seats, riders meet only through the shell's seat-to-seat contacts, which cover each
  seat's own rider. So seat 2's bike passes through the AI bikes. Seat 2 also is not among the AI
  riders' "others", so their brains don't see it. Fixing this means AI riders in the cross-seat
  contacts, or every seat in one Sim. Filed as P350a.
- **Order within a tick.** The guests are fed just before seat 0's pass, which is last in the walk
  unless seat 0 is flying. In the original, the race manager reads every rider after all of them have
  moved. If seat 0 is in the plane and another seat walks after it, that seat is fed from the last
  tick. Not checked against the original's order.
- **Caught-rider draws.** The watch draws its two numbers per caught rider in the lap list's order.
  The guests' cards come after seat 0's players, so the order of draws when several riders are caught
  on one tick is the port's own choice. It does follow from the card order.
- **A guest's clutch** for the tracker is the seat's controls of the pass (`Seat::hands`). These are
  the controls handed to the garage, not read back from inside the Sim's step.
- **Seats that join or leave mid-race** are not handled. A joiner is watched by the gate, but it has
  no lap card. A leaver shifts the later seats' race player numbers. Not tested.
- **Online races keep P313a's behaviour.** One online seat gets AI riders. Several online seats get
  no AI, no grid lanes and no shared race. The guest path is local seats only, so the online wire and
  state stay as they were.
- **The grid is laid out for max(racers, seats).** With `--racers` below the seat count, the race
  grows to fit the seats.
- **Who the seats are on the results** is each seat's `racer_name`. The race-end seat words
  (`EndSeat`) number them by seat. The original's split-screen results were not read.
- **The lap-line lines say "player N" by race player.** With two seats, player 2 and 3 are AI riders
  and player 4 is seat 2.

## Next

Landed.
