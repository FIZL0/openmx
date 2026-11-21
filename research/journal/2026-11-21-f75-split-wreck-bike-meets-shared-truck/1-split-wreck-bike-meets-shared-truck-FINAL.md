# 1 — split wreck bike meets shared truck (FINAL)

Task F75. 2026-11-21.

## What I did

- `game_sim::parked_wreck::wreck_meets_cars`: the body of `Sim::wreck_meets_parked` (S486) as a free
  function over one rider and a slice of parked cars. The Sim's method now calls it with its own
  cars, so a lone seat is unchanged.
- `Seat::pass` (main.rs) takes split screen's shared copy of the parked cars (F72). After the pad's
  poll, on the bike and riding, the seat's downed rider's wreck bike meets them through
  `wreck_meets_cars`, just before the seat's Sim steps the rider, as a Sim's own step does. The
  wreck bike is marked ticked, so the rider's frame does not tick it again. Cars are named
  `seats + k`, as the pass's hulls already are.
- Split the rest of the task off, each needing more than main.rs can hold 1:1:
  - F75a: a flown plane meets the shared cars (the Sim's plane lane steps them in the plane's
    world; split screen frames them at the pass's start; two planes flown together need a choice).
  - F75b: AI riders in a seat. The first seat's Sim frames and sleeps its cars inside its own step,
    at its turn in the walk, after the other seats' bikes pushed them. Matching one Sim's order
    needs a game-sim change.
  - F75c: the downed rider's bones (F73). Lending the cars from the shell does not work: the Sim's
    `parked_back` takes the lent list and drops each strike when its own cars are empty.
  - S499: a driven car meets parked cars. No code for it exists anywhere, alone or split, so it is
    a port that needs a capture, not split-screen plumbing.
- Opentennis: nothing like this there (no parked machines); nothing taken.

## What the original does

One dynamics world holds one truck; every rider's wreck bike searches it (S486). Split screen's seats
are one Sim's players in the original, so each seat's wreck bike meets the one truck.

## How it was proved

Against the port's own lone-seat path (itself proved against S484/S485's capture in tests/replay.rs):
`split_contact::a_split_seats_wreck_bike_meets_the_shared_truck_as_a_lone_seats` (main.rs) rides the
disc's truck at 60 ft/s alone and as seat 1 of two, and checks the wreck bike and the truck tick for
tick for 12 ticks from the wreck.

## Not verified / not 1:1

- Not checked against the original in split screen: no split-screen capture of a wreck into the truck
  exists. The proof is "same as the lone seat", which is proved against S484/S485.
- A flown plane, a driven car, AI riders and the bones still do not meet the shared cars in split
  screen (filed F75a, S499, F75b, F75c). So the test stops comparing before the bones would wake the
  truck.
- In split screen the wreck bike meets the cars at the start of the seat's pass, before its Sim's step.
  A lone Sim does it inside its step, after the steps that come before each player's frame there
  (event frame, AI cards, contacts, prop strike). None of those touch the wreck bike. Only a straight
  60 ft/s wreck with one seat downed is tested. Two seats' wreck bikes on the same car in one tick are
  not tested.

## Next

Nothing; the follow-ups are filed.
