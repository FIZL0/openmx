# S124 — tick two pads

2026-10-27. Task: `./kit plan S124`.

S89's along ride no longer parts at tick 2 on main. S107's chassis inverse inertia and S114's wheel
ray probe each parted it alone, and the two together (the S114 merge, 67d20fe9) agree. What parted
next (tick 31, the gas) was the test: it put the pads on the rows, not the ticks, so the gas came
a tick late. With S117's `save_state_inputs` both rides are bit-exact through 236 (along) and 141
(across), and the bed-edge contact lanes (106/108, 50/53) agree on every tick. Both now part in the
air, on the angular history (S179/S180's lane). Details in 1-tick-two-pads.
