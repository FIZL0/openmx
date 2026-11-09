# F31b5j — part walk served

2026-11-09. Task: `./kit plan F31b5j`.

The vehicle update's part walk (`update::Out::Walk`) used to run whole in the interpreter. Now it is
`tail::walk` on RAM (`PartWalk` in tests/mxvatv_bike_ride.rs), and C15 ride-2 and flip-1 replay
bit-exact with every port on and the RAM matching the handing run's. The walks reach four kinds of
leaf: the models, the effects and two particle emitter kinds. None of them writes a word of the bike.
The models leaf is shown inert, but the effects and emitters draw on `rand`. With them dropped, flip-1's
trick hold builds the smooth noise off a different `rand` and parts on row 114. They still run in the
interpreter; porting them is F31b5n.
