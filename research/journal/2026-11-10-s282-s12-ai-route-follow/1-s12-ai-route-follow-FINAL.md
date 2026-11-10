# 1 — s12 ai route follow (FINAL)

Task S282. 2026-11-10.

## What I did

- **Split the task.** As written, S282 covers too much for one session: `ai_tick_flag` (`./kit fn 0x00383580`,
  5936 bytes), `ai_route_follow` (`./kit fn 0x00283a10`), five vtable calls around them, the path module
  (`0x0026b410` 1076 B, `0x0026e0c0` with its net search `0x0026cbe0` 2976 B, `0x0026e4f0`, `0x003832f0`),
  and a disc format. This session did the part the brief's own note asks for (S12c's edge closure) and
  the part every later piece builds on: the path module's three queries. The rest is filed as:
  - **S282a**: the free-ride target seeker inside `ai_tick_flag` (target set +0x340, target +0x344,
    approach states +0x354, the approach-speed learning, vt+0x80/vt+0x94).
  - **S282b**: the race path follower (net locate, vt+0x78, `ai_route_follow`, the hand-over) and the
    end of the tick (flag 1's cone and the avoidance call, the slow-rider cone, vt+0x84, flag 0x800).
  - **S282c**: the track's AI path read off the disc into `ai::path::Net`.
- **Found what steers the free09 riders.** In recording2's image all three AI riders have race +0x68 = 0
  and interface +0x88/+0x8c = 0, meaning no path net. Instead they have a target set (+0x340), a target
  (+0x344) and approach state +0x354 = 1. So on free09, the target seeker (S282a) decides +0x1d0/+0x1e0,
  not a path. The path is for races (S282b/c). I wrote this into S282a's text and into `research/oracle.txt`
  (`ai_tick_flag`).
- **Ported `ai::path`** in `crates/game-sim/src/ai.rs`: `Node` (centre, two edge points, length on, next),
  `Net`, `locate` (no hint, with limit, the holding-only filter and the distance), `segment_holds`,
  `node_direction`, and `off_track`. `off_track` is the avoidance's keep-on-the-track test made of the
  three, and `ai::avoid` takes it as its `edge`.
- **Corrected the edge's gate.** `ai_update_mx` asks the path nothing, and does not push, unless flag
  0x20000 is set and all of these are non-zero: the race, the interface, interface +0x88 (path),
  interface +0x8c (node) and race +0x68 (net). See `./kit dis 0x00387ea4 0x00387ef0`. S12c's `avoid` doc
  said "or the point itself when that node has none". That is true only for a node with no next or a
  length ≤ 0. With no node at all, nothing is asked. `off_track` takes the path and node as given, and
  the caller passes no `edge` when either is missing.
- Took nothing from opentennis: it has no track path or path-following AI.

## What the original does

- **Node layout** (stride is the caller's): +0x10 centre, +0x20 and +0x30 the two edge points across the
  track, +0x40 the length on to the next node, +0x54 the next node. **Path object**: +0x10 first node,
  +0x18/+0x2c branch count/list (read only with a hint).
- **`path_locate`**: arguments are f12 limit, a0 net, a1 hint place, a2 point, a3 path object, t0 out,
  t1 distance out, t2 "refuse a 0x100 path", t3 holding only.
  - A path with one node gives that node, along 0, and distance √(mula dx² + madd dz²).
  - Otherwise it walks the stretches while there is a next node and (limit < 0 or walked < limit). For
    each stretch: t = (mula sx·rx + madd sz·rz) / (len·len), clamped (above 1 or NaN → 1, then < 0 → 0).
    d² = mula ox² + madd oz² to the clamped point. It is taken when d² < best or best < 0, so on a tie
    the earlier stretch keeps it.
  - walked += length after each stretch. With t3 set, a stretch counts only when `segment_holds` says so.
  - The distance out is best when best ≤ 0, else √best. The out node and along are left untouched when
    nothing is taken.
- **`segment_holds`**: a crossing count in x/z over the hexagon seg.edge0 → next.edge0 → next.centre →
  next.edge1 → seg.edge1 → seg.centre → seg.edge0. For each side, s = run·(px − from.x). It counts when
  `!(s < 0)`, `s < run²`, and from.z + (s/run²)·(to.z − from.z) < pz. Plain ops only.
- **`node_direction`**: returns false with no node, no next, or length ≤ 0. Otherwise io = VU0
  normalise(next.centre − centre), with `vsub.xyz` keeping the next centre's w.
- **The edge in `ai_update_mx`**: ahead = position + heading·(+0x288). It calls locate(−1, net, no hint,
  ahead, path, out, 0, 0, 0). It gives up when the node, its next or the one after that is missing, or
  when either holds test (node..next, next..after) says the point is on the track. Otherwise io = ahead,
  then `node_direction` at interface +0x8c. The push itself was already ported by S12c.

## How it was proved

`./kit check -- -p game-sim --test ai`: 10 tests pass (two new, both on the game ELF, no image needed).

- `the_path_queries_match_over_ten_thousand_paths`: 10,000 generated paths of 1–12 nodes are laid into RAM.
  The node words the queries must not read are filled with junk. A quarter of the paths use whole-number
  coordinates so points land on sides and vertices. Some have edge points swapped or folded onto the
  centre, and some have zero, negative or wrong lengths.
  - `path_locate` runs with random limits, the holding-only filter and with or without a distance out.
  - `path_segment_holds` runs on a stretch and on any two nodes.
  - `path_node_direction` runs on any node, none, or the last.
  - Compared: the answer, out's three words, the words that must stay untouched, the distance, and io.
  - Branch counts in one run: 1933 places inside a stretch, 1962 unplaced, 628 limited walks, 989 held,
    4875 directions, 4118 refused, 795 single-node paths, 1481 holding-only places.
- `the_keep_on_track_push_matches_over_ten_thousand_passes`: the original's whole `ai_update_mx` on
  generated paths, with the three queries running as the original's own code (no hooks), against
  `ai::avoid` with `path::off_track` as its edge. Flags 0x80 and 0x40 are both up and there are no riders,
  so the push is the only thing that bends the steer. Compared: steer direction (four lanes), speed, flags.
  - One run: 1567 pushed, 1588 pointed away (no push), 4033 on the track, 1429 paths too short, 1383 with
    no path or node (no edge), 1175 pushed along the point itself.
- Mutants: a tie going to the later stretch (`<=`) fails at case 5. One hexagon side walked the other way
  fails at case 2005.

## Not verified / not 1:1

- **No real track path was compared.** Every proof runs on generated paths. Reason: the only AI image
  (recording2, free09) has no path net, and no race image with AI riders exists yet. S282b's text asks for
  a 4-racer sx01 image (research/c31/boot.py); S282c compares the disc reader against one. The field
  names "centre" and "edge points" come from how the queries use them, not from data.
- **`locate`'s hint is not ported.** The hint is a place found before. A hint on the same path starts
  from its node and wraps through the branch list; a hint on another path costs the path distance
  (`0x0026e0c0`) and its net search (`0x0026cbe0`, 2976 B). Reason: nothing ported calls it with a hint
  yet; the race follower (S282b) does.
- **`locate`'s "refuse a 0x100 path" argument (t2)** is not ported. The edge passes 0. Reason as above.
- **A path object with no first node** is not modelled. The original's locate returns 1 and leaves out's
  node untouched, so the edge would read a stale stack word. The port has no such state: `Net` callers
  give a first node. Reason: not reproducible as a value, and no data is known to have an empty path.
- **A circular path with no limit** makes the original's locate loop for ever. Neither the tests nor the
  port go there. Real nets are expected to end (the branch list closes laps), but this is unverified until
  S282c.
- **The route follower itself** (target direction +0x1d0, target speed +0x1e0, the waypoint +0x44 speed,
  the rand()-drawn wobble/drift) is **not ported here**. It was split to S282a/S282b, with the reasons
  above.
- **MX vs. ATV's PC exe was not read** for these three functions before writing the proofs. Reason:
  time. They are small, pure and fully covered by the PS2 oracle, which is the binding proof.

## Next

S282a (the free-ride target seeker) is what steers the free09 opponents, so it comes before S282b/c for
anything played on free09.
