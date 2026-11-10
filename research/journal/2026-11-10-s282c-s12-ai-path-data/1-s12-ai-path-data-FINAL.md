# 1 — s12 ai path data (FINAL)

Task S282c. 2026-11-10.

## What I did

- **Found which file the AI's path is.** It is not `sx01.wpt`. It is `tracks\sx01\sx01ai.tdf`, a second
  race file in the same tag format as `sx01.tdf`. The track object reader (`./kit fn 0x00269750`, already
  ported as `race::TrackFile::parse`) reads it, and `activity_race_load` (`./kit fn 0x002f8610`) loads it
  from the activity's +0x288 name (`Tracks\sx01\sx01AI.tdf`) into act+0xbc. On sx01 it has 38 paths and
  444 nodes, a web of lines that branch. `sx01.tdf` (act+0xb8) has 2 paths and 96 nodes: a start run and
  the lap.
- **Two nets, two users.** In a racing image all three AI riders have +0xdc (the follower's net,
  `ai_tick_flag`'s path branch) = act+0xbc, the AI file. The avoidance's keep-on-the-track edge in
  `ai_update_mx` (`./kit dis 0x00387ea4 0x00387f74`) reads ai+0xd8 (the score manager, embedded at
  act+0x50). That manager's +0x68 is act+0xb8, the race file, which it asks at the card's +0x88 path and
  +0x8c node. So the brief's "race+0x68" is the race file, and S282b's follower rides the AI file. Both
  are now proved. A drop-in branch of `0x003a24f0` writes ai+0xdc = act+0xb8; that branch was not
  followed.
- **The .wpt files are something else.** `./kit fn 0x0034a660` reads a count and then 0x20-byte records
  off one. `0x003a2e90` and `0x003a39d0` read a count list off another. Neither is the AI path, and
  neither was identified further. Written into `research/oracle.txt` next to the S13e note.
- **Ported the reading into `ai::path`** (`crates/game-sim/src/ai.rs`):
  - `Net` gains `paths: Vec<Path>`. A `Path` has flags, length, its first node and its branches as path
    indices.
  - `Net::from_track(&TrackPaths)` lays each path's nodes into the flat node list. For each node: centre
    = point 1, edges = points 2 and 3, length = span, next = the following node on the same path, and
    `None` on a path's last node (+0x54 is 0 there in RAM, so a path goes on only through its branches).
  - It builds on `TrackPaths::new`, which already refuses files the queries can't walk and does the
    load's reach walk (flag 0x80).
  - The one existing `Net { nodes }` literal in `tests/ai.rs` became `Net { nodes, ..Net::default() }`.
- **Made the image** `context/states/s282c-sx01-race.ee.bin` (git-ignored; only in slot s1). To rebuild:
  load `context/states/c31-sx01-race-start.p2s` (copied from s2) into copy 1's slot 08, let it run to
  frame_counter 604, `./kit pine save 9`, then `./kit state ram` the slot 09 file. Scripts are in the
  session scratchpad; the steps are only those.
- Filed **S308**: the activity's ground snap of the path heights (`./kit fn 0x002f8b40`).
- Took nothing from opentennis: it has no track path data.

## What the original does

- `activity_race_load` reads `<track>.tdf` into act+0xb8 and, if it opens, `<track>AI.tdf` into
  act+0xbc. Both go through the same track constructor and reader. Each one then gets the ground snap
  `0x002f8b40`, `node_add_child` and +0x38/+0x3c = 125/75.
- The ground snap zeroes each node's three point heights. It then casts a ray down from 1000 (a collision
  query object, 10/30 radius setup) and writes the hit height. Not ported (S308).
- In RAM, a node's layout is +0x10/+0x20/+0x30 for the points, +0x40 for the span and +0x54 for the next
  node. A path's layout is +0 flags, +8 length, +0x10 first node, +0x18 branch count and +0x2c branch
  list. Both are exactly what `ai::path`'s queries already read (S282).

## How it was proved

`crates/game-sim/tests/ai.rs::sx01s_path_nets_off_the_disc_are_the_race_images`, run with
`./kit check -- -p game-sim --test ai -- sx01s_path_nets`.

- Both files are read off the disc (MX.PSR → `Tracks\sx01_2.res`), parsed and made into a `Net`.
- Each net is compared against the image's net at act+0xb8 and act+0xbc, node for node:
  - for every path: the path count, flags (after the reach walk), length bits, first node, and branches
    (as path indices);
  - for every node: the x/z/w bits of the centre and both edges, the length bits, and the next link (as a
    node index, with `None` ↔ 0).
- Who uses which net: each of the three AI riders has +0xdc = the AI net, and +0xd8's +0x68 = the race
  net. Each rider's card +0x88/+0x8c lies on the race net, and the follower's spot +0x220/+0x224 lies on
  the AI net (at least one rider has one).
- 10,000 calls of the original's own `path_locate` on the image's real nets (5,000 per file):
  - The points are within 60 ft of a random node's centre or edge. A third of the calls have a walk
    limit, and a third use holding-only.
  - Each is checked against `path::locate` on the nets off the disc: the answer, the node, the
    along-fraction bits and the distance bits.
  - 7,775 of the 10,000 were placed (checked with a temporary probe panic).
- Mutant: swapping the two edges in `from_track` fails at "sx01.tdf: node 0".

## Not verified / not 1:1

- **Heights.** The node heights the port keeps are the file's; the original snaps them to the ground
  after loading (`0x002f8b40`), so the test compares x, z and w only. Reason: the snap is a collision ray
  query, which is out of this task's scope. `locate` and `segment_holds` read only x and z, but
  `node_direction` (the follower's and the edge's push direction) reads y. Filed as S308.
- **Only sx01 was compared.** Reason: there is one race image. Other tracks' `AI.tdf` files go through
  the same reader (proved over generated files in tests/race.rs, S13e), but no other track's net was
  compared to RAM.
- **`TrackPaths::new` refuses** an empty path slot, a path with no nodes, a link to no path, or a path
  without the owned flag. The original would read memory it never allocated in those cases. No retail
  file is known to have them; sx01's two don't.
- **The .wpt files' contents and readers** (`0x0034a660`, `0x003a2e90`, `0x003a39d0`) were not
  identified. Reason: they are not the AI path, which is what this task needed.
- **The drop-in branch** of `0x003a24f0`, which points a rider's +0xdc at act+0xb8 (the race file, not the
  AI file), was not followed. Reason: it was not reached in the image. The port's caller has to choose
  the net, and S282b should take this branch into account.
- **The follower's spot +0x220 layout** (path, node, along) is assumed from `path_spot`'s out argument.
  The test checks only that the path and node words lie on the AI net.

## Next

S282b (the race path follower) can now take `Net::from_track` of the AI file as its net and the race
file's as the edge's net. Use `s282c-sx01-race.ee.bin` for its real-net checks. S308 snaps the heights.
