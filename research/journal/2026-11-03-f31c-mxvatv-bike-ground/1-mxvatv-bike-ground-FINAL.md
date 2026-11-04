# 1 — the ground under C15's rides (FINAL)

Task F31c. 2026-11-03. Slot s3.

## What the bike asks of the ground

I traced the F31 replay of ride-2 and flip-1 (pass-through hooks on every ground and search call; the
trace test was scratch and is gone). Addresses are in research/oracle.txt `# F31c`.

- **The wheels.** Each substep (four a frame) the bike's ground step searches its two trigger rays
  over the nearby list. Then each wheel's suspension asks the ground query (MD76's, already ported as
  `atv_ground::query`) at its probe end, with the wheel's `+0x110` as its record byte. Ride-2 row 0
  makes 8 ground queries and 4 trigger searches.
- **The collision record.** Once a frame, before the world step, the record object runs its pass. It
  searches its sphere probe. Then, for each down-pointing ray whose node is over the map, it asks a
  second terrain query, which hands back the point's height, the blended normal, the cell's surface
  word and the four corner weights. The terrain service reads these back field by field. Then it
  searches its rays over the nearby list (once in row 0). Only flip-1's wreck rows (101–124, 193) take
  the terrain branch: 470 calls.
- **The props.** No search of the bike's probes files a contact on either ride (the record count stays
  0). The plants' spheres set only the hit bit. The captures agree: the wheels' swept contact (+0xd4) is
  0 on every row and terrain-wins (+0xd8) is set on every row.

## Decision: the ground is the terrain

Everything in C15's rides that answers the bike's ground questions is the terrain. So F31c ports the
terrain side whole and checks that the searches file nothing, instead of building Nat05's prop world
(scene, cols, the esb's plants) for searches that return empty. F31b can ride on this. A ride that
touches a prop needs the world, and that is filed as a follow-up.

## What I did

- `crates/game-sim/src/handling/ground.rs` (new, `pub mod ground` in handling.rs):
  - `terrain_file(res, map)`: the map's `Env\<map>\<map>.ps2.trn` out of its `.res`, any case;
  - `terrain(map)`: `atv_ground::Terrain::of`;
  - `read_terrain(words, at, map)`, with its inverse `terrain_words(t, at)`, as `seed`'s readers are;
  - `under(t, &mut Ground)`: the record's terrain query on `contact_search::Ground` (point, normal,
    surfaces, weights). Its disassembly holds the point in the box, picks the cell, blends the normal
    and works out the height exactly as the wheels' query does (the same `adda`/`madda`/`madd` chain,
    the same `vadday`/`vmaddz` length). Only its outputs differ. With no cell it writes the normal up and
    the surface word 0, and nothing else;
  - `surface(&Ground)`: the service's pick, the first corner with the largest weight over 0;
  - `WHEEL_TERRAIN` (+0x1e0), `WHEEL_RECORD` (+0x110).
- Nat05's `.res`, copied from openmx-mods' extracted disc into `context/mxvatv-ps2/nat05/`
  (git-ignored).

## How it was proved

`tests/mxvatv_bike_ground.rs` (`./kit check -- -p game-sim --test mxvatv_bike_ground`: 5 passed):

- `state4s_terrain_is_nat05s_map`: both wheels hold the one terrain object, and every word
  `terrain_words` gives for `terrain(Nat05)` is the image's. `read_terrain` round-trips.
- `ride2s_first_frame_wheel_queries_are_the_ports` (the task's check): ride-2 row 0 replayed with the
  queries passed through. 8 wheel queries (4 front), each answered by the port with the same point,
  normal and record byte bit for bit. Searches per probe [trigger 4, wreck 0, riding 1], none filed.
- `every_wheel_query_of_ride2_is_the_ports`: all 459 rows, 3672 wheel queries, none filed.
- `every_ground_query_of_flip1_is_the_ports`: all of flip-1, wreck and put-back: 3722 wheel queries,
  and 470 record terrain queries with their whole buffers (point, normal, surface word, weights). None
  filed.
- `the_records_terrain_query_is_the_originals_over_nat05`: 6000 points over the map and up to 320 ft
  past its edges, the ring cold, some coarse or parity-split. The original's query and the port's
  leave the same 16 buffer words, and the service's surface pick equals `surface`. More than one
  surface is seen.

`./kit audit` fails only on HEAD's existing leaks (atv_wheel.rs, plan/DONE.md). New files add none.

## Not verified / not 1:1

- Nat05's prop world (scene props, cols, the esb's plant groups) is not built or compared. The port
  has no search for the bike's trigger rays or its record's rays and sphere probe. What is checked is
  that the original's searches file nothing on these two rides. A ride into a prop or a plant needs it.
- The plant spheres' hit bits (set with no contact) are not compared. The replays showed nothing on
  these rides that reads them, but that was not proved word by word.
- The record pass's other steps (the node-under-ground test, contact filing, the service calls besides
  the surface pick) are traced, not ported. F31b ports the pass.
- The other object that searches and asks the ground around row 215 (from FUN_0049bb00) was not
  traced further. It is not the bike.
- The query's ten-cell ring is not ported (as in MD76): it changes no answer.
