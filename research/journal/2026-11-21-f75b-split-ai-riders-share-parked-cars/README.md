# F75b — split ai riders share parked cars

2026-11-21. Task: `./kit plan F75b`.

Split screen with AI riders in the first seat's Sim now shares one copy of the parked cars, as F72 does
without them: the shell frames the cars before every bike and sleeps them after all, and lends them to
that Sim for its pass with the new `Sim::parked_cars_held` raised, so its step strikes and pushes them
but neither frames nor sleeps them. Proved by a disc test: seat 1 pushing the truck beside a far AI rider
moves it bit for bit as with no AI rider. Details in 1-…-FINAL.md.
