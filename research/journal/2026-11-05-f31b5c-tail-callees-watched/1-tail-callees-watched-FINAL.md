# 1 — tail callees watched (FINAL)

Task F31b5c. 2026-11-05.

## What I did

- **A census in `tests/mxvatv_bike_ride.rs`** (F31b5c section, end of file):
  - `label` names the interpreter calls that follow by their `Out`'s variant.
  - `census` wraps the interpreter call in `TailInRam::run`, `WreckInRam::call` and
    `UpdateInRam::call`. When a census is being taken, it snapshots the bike object (`+0..+0x1800`)
    and the physics blocks (`physics`: body, node, record, front, rear, engine) before and after the
    call, and files every changed word under the label and the row.
  - `census_of` is `replay_steps` (F31b5a's frame, still bit-exact on every row) with the census
    taken.
- **Ported** (`game_sim::handling::wreck`):
  - `engine_reset`: idle revs, two zeros, then `engine_gear(1)`, which is `move::set_gear` on the
    engine's words: `mul.s` then `div.s`;
  - `wheel_reset`: eight zeros, six frames to the origin, two to `WHEEL_REST`.

  `WreckInRam` now serves `Out::Engine` and `Out::Wheel` with these, and `Out::Sync` with F39's
  `update::sync` (after checking vt `+0x378` is the sync). The unwreck and the put-back reach them, so
  every replay in the file that runs the port's tail now uses them.
- **Split off**: F31b5e (the ground reset in Rust) and F31b5f (the bike words that the callees write).
  F31b5 re-blocked after F31b5b..F31b5f.
- research/oracle.txt `# F31b5c`: the resets, the ground reset's and orient's calls, and the census'
  findings.

## What the original does

- Engine reset (`./kit dis -p atv 49f7f0`): `+0xa0 = +0x54`, `+0xa4 = +0xa8 = 0`, then a tail jump
  to the gear set (`./kit dis -p atv 49fb50`) with 1. `blez`, then `slt` against `+0x6c` (signed).
  After that `+0x5c = g`, and the ratio is `+0x34 · table[g−1]`, `+0x5c` read back for the index.
  `+0x74` gets the ratio and `+0x78` gets `1.0 / ratio`.
- Wheel reset (`./kit fn -p atv 4a04c0`): plain stores, quads from the two rodata frames.
- Ground reset (`./kit fn -p atv 492ca0`) and the orient it calls through the bike's vt `+0xf0`
  (`./kit fn -p atv 492e50`): see oracle.txt. It is a tree of VU work and body/node virtuals.

## How it was proved

- `the_engine_and_wheel_resets_are_the_originals`: 20 000 cases on state 4's engine and both wheels,
  10 000 for each reset. The randomized words:
  - the engine: idle, gear count (≤ 0, `i32::MIN`, 0..6), final drive and ratio (±0, 1, denormals,
    ±max, any bits);
  - the wheel: every word.

  SLUS-21104's resets and the ports leave the engine, its table and both wheels word for word the
  same. Paths: engine in first, engine with no gears, wheel (each ≥ 1000).
- `the_rides_tail_callees_leave_the_physics_alone`: ride-2, all 459 rows bit-exact. Its 23
  interpreter callees (`RIDE_CALLEES`) change no word of the physics blocks.
- `the_flips_tail_callees_leave_the_physics_alone_but_the_ground_reset`: flip-1, all 465 rows
  bit-exact. Of its 44 callees (`FLIP_CALLEES`), the only one that changes the physics blocks is
  `wreck Ground`, on row 193 only. It writes body `+0x40..+0x1f8` and node `+0x30..+0xa8`.
  - Before the ports, the census also caught `wreck Engine` (engine `+0xa0`/`+0xa4`) and
    `wreck Wheel` (both wheels' frames and zeros) on row 193. They are gone from the list now.
  - The callee lists are pinned, so a new path in the interpreter shows up as a failure.
- Bike-object words written, seen in the census and not pinned by an assertion:
  - the node calls' outputs (frame `+0x750..+0x7c8`, position `+0x7f0..+0x828`, axes
    `+0x9f0..+0xa08`), which the port reads back as their answers;
  - the sender `+0xcfc`;
  - trick hold `+0x14dc..+0x14e8` (rows 113–124);
  - kind set `+0x9a8`, ground `+0x60..+0x7c` (row 193);
  - the event post's `+0x460`/`+0x468` and settle `+0x1754` (row 125).
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: 35 passed (every earlier replay stays
  bit-exact with the resets ported). `./kit audit` ok.

## Not verified / not 1:1

- **The ground reset is not ported** and changes the physics blocks (flip-1 row 193). It is the one
  callee that is neither Rust nor shown inert. Filed as F31b5e: a VU and virtual tree too large for
  this session.
- **"Inert" means the physics blocks only.** Several callees write bike-object words (listed
  above). The trick hold's `+0x14e4` is the pose blend that the wreck test reads. Nothing here shows
  that dropping those callees keeps the next frame exact. Filed as F31b5f. The node outputs and the
  sender belong to F43 and F44.
- **The census only covers what the replays reach.** Neither replay runs:
  - the tail's own listeners' walk, wreck first/pose/last, fade over, put-back slot, trick
    start/end (the tail's), or the run-over calls: the state's kind is 1 on every row, as F31b4a
    found;
  - the update's net interpolate/post, its wheel clear, contacts, suspension or tire forces;
  - the wreck's bounds, rand or body damping.

  Those are not shown inert. The physics callees among them (wheel clear, contacts, suspension, tire
  forces) are certainly not inert.
- **A callee's other effects are not watched**: only the bike object and the six blocks. RAM
  elsewhere (pad, message queue, animation and scene state) is the interpreter's, and the replays'
  physics stay bit-exact with it.
- **Per call, not per word read**: a callee that changed a physics word and put it back within the
  same call would not show. The census compares before and after each call.
- **The resets' oracle** draws the engine's ratio table as one word at the table's first entry,
  since the gear is always 1. A gear-set with another `g` is in `engine_gear`, but nothing calls it
  with another `g` and no oracle tests it.
- **`f12`** is handed as 0.0 in the oracle, and as dt by the tail. Neither reset reads it (by the
  disassembly).

## Next

F31b5e (the ground reset) and F31b5f (the bike words) before F31b5. The census (`census_of`,
`census_writers`) is the tool for both: port a writer, re-run, and its label leaves the list.
