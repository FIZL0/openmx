# 1 — trail sender part virtuals

Task F44. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/update.rs`: ports of the virtuals F39's `sync`, `network` and `parts`
  called out, as state 4's objects fill them:
  - `trail_on` / `trail_off` (the trail's vt `+0x18` / `+0x20`): the node's two hidden bits cleared or
    set, one byte store each, then its own shown / hidden. `shown` / `hidden` (vt `+0x28` / `+0x2c`):
    the walk of the node's children, each one not kept apart (bit 3) shown / hidden in turn. New
    `Out::Shown(node)` / `Out::Hidden(node)`.
  - `send` (the net sender's vt `+0x20`, a thunk to the bike's own): the stunt packet on beats 3, 6, 9,
    then the whole state on beat 0 or the change on 1-9, the beat on by one mod 10, answering 1. New
    `Out::NetStunt(arg)`, `Out::NetFull(arg)`, `Out::NetDelta(arg)` for the three packets.
  - `part_step` (a part's vt `+0xbc`, both state 4 parts the same code): `+0x126` bit 4 cleared, then
    `Out::Walk(part + 0x24)`.
- `tests/mxvatv_bike_ride.rs`: `UpdateInRam::out` runs these ports wherever the object's slot holds the
  original it stands for (so a class with other code still runs its own), and maps the five new
  `Out`s to their originals. The census lists `RIDE_CALLEES` / `FLIP_CALLEES` follow: `update Part`
  and `update Send` leave the interpreter, `update NetDelta`, `NetFull`, `NetStunt` are what is left.
- New oracle `the_trail_sender_and_part_virtuals_are_the_originals`.
- research/oracle.txt `# F44`: every address, slot and word.
- opentennis: nothing to take; its netplay is ggrs over iroh, not this game's packet sender, and it has
  no scene-node tree like this one.

## What the original does

`./kit dis 0x00128040 --len 64 -p atv`, `./kit dis 0x00128080 --len 64 -p atv`, `./kit dis 0x00127f60
--len 0x70 -p atv` (and `0x00127fd0`), `./kit dis 0x004bfa30 --len 16 -p atv`, `./kit dis 0x004e7de0
--len 0x88 -p atv`, `./kit dis 0x004a8d90 --len 0x9c -p atv`, `./kit dis 0x001bcfa0 --len 0x24 -p atv`
(and `0x001bacd0`). The vtables were read off the state04 RAM image: the trail and every node under it
(22 children, 4 classes) share the on/off/shown/hidden slots; the sender's slot is the thunk; the
record's and the second part's `+0xbc` are two copies of the same nine instructions. In words:

- On/off: clear (set) bit 0, store; clear (set) bit 1, store; tail-jump to the node's own shown
  (hidden). Shown/hidden walk the children list (first `+0x10`, next `+0xc`, the next read after the
  call), skipping a child with `+0x21` bit 3 set. The base class's shown/hidden write nothing, so on
  state 4 an on or off writes only the trail's own byte.
- Send: `+0x414` bit 8 and the beat (`+0xcfc`) 9, 6 or 3 → the stunt packet (vt `+0x544`). Then a
  direct call: `+0x414` bit 8 read again; beat 0 → the whole state (`+0x400`), beat 1-9 (unsigned
  `< 10`) → the change (`+0x404`), anything else nothing; the beat read again and set to `(beat + 1)
  rem 10` (signed); answers 1. With the bit clear only the answer.
- Part step: `+0x126 &= ~0x10`, then the listener walk of `part + 0x24` with `dt` still in f12.

## How it was proved

- **Function oracle, 10 000 cases**: `the_trail_sender_and_part_virtuals_are_the_originals`, on the
  state04 image (it asserts every slot above holds the function named). Each case runs the original
  from the same words and then the port through `UpdateInRam` (the replays' dispatch, so the slot
  checks are exercised too), with the same hooks logging below both. Trail cases: the trail's `+0x20`
  at random and a random tree of 0-14 nodes under it, each state 4's class or one in four a class with
  hooked shown/hidden, `+0x20` at random. Send: the sender's bit on or off, the beat −2..12, 0 one time
  in four or any word one in eight; the three packet hooks flip the bit (1 in 5) and set the beat (1 in
  4) off a hash of what they find, so the re-reads matter. Part: either part, `+0x124` at random, the
  walk hooked. Every word of the bike (0x1820), the trail, the tree and both parts (0x130 each)
  matches, the calls match in order with their arguments (each hook asserts f12 = dt, the packets a0 =
  the bike), and so does the send's v0. Paths, each ≥ 129: trail a tree 4715 / no nodes 326, a node
  kept apart 4411, hooked nodes reached 1789, no change 1272; send off 1223, beat out of range 430,
  stunt 169, whole 305, change 438, stunt then another 129; record's part 1230, second 1293.
- **Mutants**, each failing the oracle (reverted): the stunt beats `3 | 6 | 9` to `3 | 6` (case 194, the
  calls), the kept-apart bit 8 to 4 (case 0, the calls), the part's bit 0x10 to 0x20 (case 3, a word),
  the beat's `rem 10` to `rem 9` (case 51, a word).
- **Replays**: `ride_replays_bit_exact_on_the_ports_record_pass` and
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass` stay bit-exact under F31b4a's
  RAM-wide shadow check with the ports in the tail. A temporary print counted on ride-2 153 sends
  through the port (each beat 0-9 15 or 16 times, with the stunt packet on 3/6/9 and the change on the
  rest), and both parts stepped by the port on every frame; the census counts 47 stunt packets on
  flip-1. `the_rides_tail_callees_leave_the_physics_alone` / `the_flips_…` pass with the new lists.
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: 48 tests, 46 passed and the 2 census tests
  failing on the old lists, before the lists were updated; then those 3 tests passed. The whole gate
  runs in `done`.
- A trap met on the way: my mutant loop put update.rs back with `mv` from a copy older than the last
  build, so cargo kept the `rem 9` mutant and the replays failed on `+0xcfc` until the file was touched.
  Not a port bug; restore with `cp` + `touch`.

## Not verified / not 1:1

- **The trail's on and off never run on a replay.** On state 4 and both C15 replays the bike's flags
  (`+0x444` 0x18) never hold the replay's mask (`+0xa94` 0x20), so `sync` never reaches the trail: 0
  calls. `trail_on`, `trail_off`, `shown` and `hidden` are proved by the oracle only, on random trees
  of state 4's node class and a hooked one.
- **The three packets are still the originals** (vt `+0x544` the stunt's, `+0x400` the whole state,
  `+0x404` the change, with their quantisers and the sender's own send they end in). The oracle hooks
  them, so it proves the send's dispatch and beat, not the packets. The stunt packet reads the RCNT0
  hardware counter and `hooked()` stubs it on the replays, as before F44. Follow-up F44a.
- **The node classes other than state 4's** (a trail or child whose on/off/shown/hidden slots hold other
  code) run their own originals through `UpdateInRam`; the oracle stands one in with hooks. Only state
  4's classes are read; another track or bike could hang other classes under the trail.
- **The walk inside `part_step`** is `Out::Walk`, which `UpdateInRam` hands to the original
  `0x00128af0` (as F39's `parts` did); the oracle hooks it. `tail::walk` is its port elsewhere.
- **A sender other than the bike's** (`Out::Send` on another object, or one whose slot is not the
  thunk) still calls the original; the port is the bike's send only, as the thunk is.
- The oracle's random `+0x20` / `+0x124` words go through the image's other bytes of those words too;
  the port writes them back as read, which the word comparison checks, but their meaning is not.

## Next

None for F44. Follow-up F44a filed for the three packets.
