# S276 — the wreck plane in the lanes

2026-11-03. Task: `./kit plan S276`.

S274 ported how a wrecked rider's wreck bike meets a flown plane (`game_sim::wreck_plane`), but only
`tests/c16_fly.rs`'s split-screen pass ran it. S276 wires it into:
- `Sim::step`'s plane lane;
- the shell's plane loop (lone plane);
- online's plane loop (lone plane).

`fly()` can now also seed P2 down. Planes flown together are left for S276a. Part 1 has the detail.

## Not verified / not 1:1

- **No capture pin could be re-run.** C16's capture and tape (`context/capture/c16/fly.csv`,
  `context/recordings/c16/fly.p2m2`) are in no slot or kept-context, so the JOUSTS and STRETCHES tests
  skip. The new tests use only the s272 RAM images and pads rebuilt from research/c16's segment list.
- **STRETCHES[6] (137910) stays 0.** With P2 seeded down, the first tick is S274's (-24.854322, 8.510814,
  2.1630852) against row 137911's (-24.94349, 8.60006, 2.21980). The plane's resolve against the wreck
  mirror is still not bit-exact. That is S274's open question, not this wiring's.
- **Planes flown together do not meet wreck bikes** (`fly_together`, `fly_held` in the shell and online).
  This is follow-up S276a.
- **In the Sim lane only the wreck bike ticks among the plane.** The rest of the downed rider (its body
  and the rider's own tick) is still not stepped there. The plane lane never stepped the other riders, as
  before.
- **The shell and online paths are checked only by the build, and by `fly_two_by`'s copy of them.** No
  test drives the shell or a netplay ride into a downed rider's wreck bike. The netplay browser tests run
  rides, but none with a plane over a wreck.
