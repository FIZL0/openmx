# F31b4b: the bike's wreck around the sim frame

2026-11-04. Task: `./kit plan F31b4b` (filed by F31b4).

The vehicle update asks the bike's wreck test every frame, and the update's put-back slot puts it back
at its `+0xd38` spot. Both, with the entry, the unwreck, the riding part's toggle and the collision
record's part swaps they call, are now `game_sim::handling::wreck`. `tests/mxvatv_bike_ride.rs`'s
`TailInRam` serves the tail's `WreckTest` and `PutBack` with the port (`port_wreck`, through
`WreckInRam`) in place of the original's. Addresses, slots and offsets are in research/oracle.txt under
`# F31b4b`.

## What it does

- `wreck::wreck_test(at, dt)`: for a bike someone drives with wrecks on, out of bounds (or the handed-in
  position's bounds test) enters 13 and stops; otherwise, not yet down, the clauses (`wreck::clauses`,
  in `step::wreck_test`'s order), **then the hits and the striker whether or not a clause entered**
  (`wreck::hits`); the striker word cleared; the clock (`wreck::clock`: the count armed past 2.5 s for
  13 or 0.5 s, the unwreck past `MaxWreckTime`). A bike nobody drives is wrecked with 14. Then the run's
  riding bit and the wreck count.
- `wreck::enter`, `unwreck`, `part`, `swap`, `put_back`: the bike's own virtuals. Each is an `Out` when
  another calls it, so each is proved on its own; `WreckInRam` serves them with the port.
- Every word goes through `Wrecks::word`/`set` in the original's order, every call out is a
  `wreck::Out`.

## Decisions

- A word-level port, not `step::wreck_test` / `hit_wreck` reused: the original runs the hits and the
  striker after a clause's entry (so a frame can enter two or three times, and the last reason and throw
  stay), its comparisons keep the original's NaN senses, and it touches many words the typed step does
  not hold. The typed step's difference is filed as a follow-up.
- **MD63f's wreck gravity is not in state 4's wreck.** The two virtuals that rewrite gravity
  (`0x004d3f90`, `0x004d40a0`) are in vtable `0x003cfad0`, which no state-4 object uses. The state-4
  bikes' entry and unwreck touch gravity nowhere; the one word rescaled is the body's `+0xe8` target's
  `+4`, divided by the throw scale on a 13's unwreck. Filed as a note.

## Checked

- `the_wreck_test_is_the_originals`: 12 000 random bikes (driver, kind, wrecks on/off, out of bounds,
  reasons, positions incl. the origin, MD63c's limits and loads at and past their edges, pitch, roll,
  air words, stunt ids, pose blend, the record, contacts, actors, the striker, the clock about its
  windows, the run) through 0x004e24a0 and the port, every callee hooked to log and answer. RAM (bike,
  body, the fake world, the globals) and the call log match. Counts: no entry 8070, 1 front 211, 2 left
  120, 3 right 242, 4 back 132, 5 flat or struck 814, 6 scenery 70, 7 joust 152, 8 nose dive 48, 11 tail
  296, 12 no-hander 301, 13 out of bounds 324, 14 no control 1354, 15 struck 310, two entries in a frame
  442, three 25, count armed 868, unwrecked 542, a position's bounds asked 339.
- `the_wrecks_virtuals_are_the_originals`: 12 000 direct calls of the entry, unwreck, part on/off, both
  swaps and the put-back on random words. RAM and calls match. Counts: entry 1261, entry 0 245, entry 13
  541, unwreck 917, unwreck 13 1008, part on 1031, part off 979, to the wreck part 1962, to the riding
  part 2022, put back 1367, put back snapped 667, rand 541, rumbled 295, wreck damping 1031, attached
  2921, bone placed 1364, ground reset 2034, trail reset 1380.
- `ride_replays_bit_exact_on_the_ports_record_pass`, `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass`:
  the hybrid frame now runs the port's wreck test inside the port's tail, under F31b4a's RAM-wide shadow
  check of the bike's update. Both stay bit-exact; flip-1 enters 1 (front) on row 125 and the clock's
  unwreck puts it back on row 193 (a temporary print confirmed the port ran those). The tail's own
  put-back slot is never called on either replay; the virtuals oracle covers it.
- Mutants: the pose blend's `<=` to `<` fails the test oracle (case 64); the contacts' reach `<=` to
  `<` fails it (case 70); the put-back's lift 0.3 to 0.25 fails the virtuals oracle (case 56, 3 words).
