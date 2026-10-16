# S20 — s20 animation caller

2026-10-16. Task: `./kit plan S20`.

The four things S6 left on its own "not verified" list, which turned out to be four pieces of one thing:
the step between the rider's animation selection and the motion library. The clip table each state asks
a handle out of (built by name, gated by `UsedForBike`, with the three airborne poses as named fields);
the one function in the game that starts a clip-to-clip transition, which is the only writer of the
`Fade::on` flag and is never called by the rider itself; the pose snapshot a cross-fade fades from,
which S6 stubbed; and blend mode 3, which does not exist — the dispatch has no arm for it, and the
blocks Ghidra drops belong to the vehicle's copy of the same dispatch, where they set mode **4** and
call a four-clip entry. All four are ported and proved bit-exact by four new oracle proofs (5 to 8) in
`crates/game-sim/tests/motion.rs`. Two S6-era mistakes fell out of the reading and are fixed: the
motion object's +0x4c is the clip a transition is coming out of rather than a sampler selector, and the
"a sample time of exactly zero fires nothing" rule belongs to the one-clip entry alone.
