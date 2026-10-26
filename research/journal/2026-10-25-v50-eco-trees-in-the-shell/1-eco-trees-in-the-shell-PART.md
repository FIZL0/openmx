# 1 — eco trees in the shell (PART)

Task V50. 2026-10-25.

## What I did

- `game_sim::eco::Trees::on_ground` (eco.rs): parse an `.esb`, stand the plants that ask on a
  `world::Ground` with the terrain object's point query, and build the trunks on a cell of the map's
  larger side over 65536. The extent is the field's samples × spacing, the same sum
  `tests/flight_candidates.rs`'s `free06_trees` and the renderer's grove (V48) use.
- `game_sim::tiles::Land::Woods(Box<Wooded<Place>>)` (tiles.rs): a track with its trees. `World` goes
  through `Wooded` (so `groups`, `crowded` and `wooded` see the trunks); `Surface` and `Obstacles`
  (the camera) go to the track alone. `Land` now forwards `World::wooded` too; for `Track` and
  `Tiles` that is the same answer the default gave before.
- `crates/openmx/src/main.rs`: after the props, the scene's `EcosystemFile` is read from the track's
  archives and built into trees (`trees: N trunks under M plants`); the track becomes `Land::Woods`
  when it has trees. A world mod (MD8) gets none: its tiles' plants are drawn per tile (MD15) and
  `Tiled` has no tree list.
- `crates/openmx/tests/trees.rs` (new).

Nothing taken from opentennis: tennis has no trees or placed groups.

## What the original does

S97's journal has it: the scene loader asks the ecosystem loader to build solids on a free ride, and
the bike's body rays search the placed groups. Nothing new was read for this task.

## How it was proved

`crates/openmx/tests/trees.rs`, `the_shell_rides_into_a_free09_tree_and_stops_at_its_trunk`, runs the
shell headless on free09 off the disc:
- the shell's `trees:` line names as many trunks and plants as the disc's `free09.esb` has (630, 726);
- it picks the trunk nearest the start line that has no other trunk within 40 ft (the one at
  4068.1, 3401.2) and rides at it from 40 ft back, full gas, `--trace-feel`: the bike goes from 56 ft/s
  to 7 ft/s in one tick, centre of mass 2 ft short of the trunk's face, and gets no further than
  38.5 ft of the 40;
- the same ride 10 ft to the side gets 318 ft along.

## Not verified / not 1:1

- **Not compared with the original.** The original was not ridden into this tree (same gap as S97:
  no pad here, and a teleport plus generated-tape run was out of scope). The test shows the shell
  stops at the trunk. It does not show it stops where the original does.
- From 80 ft back the bike drifts 2.5 ft sideways on the slope and passes the trunk, 1 ft clear of its
  face, without touching it. The port's contact bundles stand in for the original's body rays (S97's
  note), so a near miss may go differently in the original.
- After the stop, the bike under gas slides round the trunk and **wrecks** about 8 ticks later
  (frame 45 of the 40 ft ride). S97's flat-floor test saw no wreck. I did not look into whether the
  wreck comes from the trunk or from the slope, or whether the original wrecks too.
- The extent (samples × spacing in `F`) is the sum the other loaders use. Only free09's 6912 was
  checked against the original's cell size (S97). Plants are stood with `Ecosystem::stand`, which is
  not proved (S97's note); free09 has no plants that ask to be stood.
- A world mod's tiles have no trunks in the simulation; only their pictures have trees.
- The wreck bike and the biplane now see the trunks in the shell through `World::wooded`
  (S102's searches). No shell test covers that path.

## Next

The work is done. It is not ticked because the gate is red only on main's S119:
`./kit done` (2026-10-26) failed on one test, replay.rs `the_slip_cases_replay_from_the_save_state`
(S58 'turn' parts at 75, not after 82), the same red P74 recorded on the commit just under this
branch's base. The test does not reach `Land` or `eco`. I did not rerun it on bare HEAD in a scratch
worktree; P74's run on this base's parent stood in for that. Every other test in the gate passed,
`trees.rs` included. Committed and `task/V50` pushed by hand. Once S119 lands, rerun
`./kit done V50` to tick it.
