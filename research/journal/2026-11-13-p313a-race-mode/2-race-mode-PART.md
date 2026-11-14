# 2 — race mode (PART)

Task P313a. 2026-11-14.

## What I did

- Picked the task up again now that S12e has landed (108707956). Checked what S12e left for the shell
  to call. **Nothing builds an `ai::Lane` outside a test.** S12e's replay
  (`crates/game-sim/tests/ai/race4.rs`) builds it from C33's RAM image:
  - `Lane::tables` comes from `controls::tables(&ee)`, which reads the curve table out of RAM;
  - `Lane::seek` comes from `sim.rs`'s `s12e_seek_world`, which reads hit.dat off the disc but takes
    each set's owner (kind, player, node position) from the actor list in RAM;
  - each `Pilot` is read out of the brain in RAM (`c33_pilot`), not `Pilot::spawn`ed, and its `Seat`
    words (speed scale, slow radius and speed, air cone, look-ahead, bump chance, race phase, path
    net, live) all come from the interface and rider structures in RAM;
  - the bikes are seeded at C33's bodies, not at the grid's slots.
- The spawn at a real map load (the roster to Pilot::spawn per racer, seats, the seek world, grid
  slots) is **S331**, which S12e filed and which is still open (`./kit plan S331`); nothing on main
  touches `tick.rs` past S12e.
- If the shell made up those words, a `--race --racers 4` ride would have AI bikes, but their seats
  and seek world would be guesses rather than the original's. That is the "close enough" the project
  rules out for simulation, and it is S331's work, in S331's file. Wiring only the COMPUTER_CARD
  entries now would give cards to riders that do not exist. Neither half can pass the brief's test
  (four cards and three AI riders behind the gate) until the lane can be spawned.
- Blocked `--after S331`, and noted on S331 that this user-reported bug ("on release, there are no
  bots") waits on it.

## What the original does

Not read this session (nothing was ported).

## How it was proved

Nothing was proved: nothing landed.

## Not verified / not 1:1

- The whole task: there are still no AI riders on the shell's grid and no AI cards in race_start::laps.
  The reason is that `ai::Lane` can only be built from a RAM image (tables, seek-world owners, pilot
  seats). S331 has to port the load-time spawn first.

## Next

Once S331 has landed, use its spawn in the shell's Race when racers > 1. The grid spots for lanes
1..racers come from race_start::lay_out's `Start::activity.grid_spot`, unless S331 already places the
bikes itself. Push a `(spot, race::COMPUTER_CARD)` per AI pilot into the `riders` passed to
race_start::laps, in the Sim's player order. Then test it in crates/openmx/tests/race_menu.rs
(`--race --racers 4`: four cards and three AI riders behind the gate) and check it in a release build.
