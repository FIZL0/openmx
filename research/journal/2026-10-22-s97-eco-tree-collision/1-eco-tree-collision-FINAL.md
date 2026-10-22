# 1 — eco tree collision (FINAL)

Task S97. 2026-10-22.

## What I did

- `crates/game-sim/src/eco.rs` (new): the .esb parse the sim needs (species solids, plants), the
  plants' stand on the ground, the kind-1 cylinder (16 sides, top ring then bottom ring, 32 triangles),
  a bit-exact port of the engine's run-time collision-tree builder (`eco::build` → nodes, facets with
  packed normals, the transformed vertices), and `Trees`: one S76 `Prop` per plant per trunk, swept
  with `Props::sweep`, plus the put-back "crowded" test against the trees. `Wooded<W>` wraps any `World`
  and adds the trees.
- `World::groups` (world.rs, default `false`): sweeps a bundle against placed groups only. `ride.rs`
  calls it next to `solid` in `contacts`, `hits` and `hits_among` (the bike's own probes; see below).
- `ps2-oracle` Ee: MMI `pexew` and `prot3w`, which the builder uses.
- `research/oracle.txt`: the S97 block — loader, cylinder, builder parts, group parts, the free09 facts.

## What the original does

The scene loader passes "build solids" to the ecosystem loader (`./kit fn 0x0012acb0`). For each
species that has solids, `./kit fn 0x00129d80` turns each 0x30-byte solid record into a collidable.
Kind 1 is a cylinder: 16 sides from the curve table at a step of 2π/16, radius r, from y 0 to the
height, laid into a stack mesh collidable and given to `./kit fn 0x001ed430`, which copies the
vertices and calls the tree builder (`./kit fn 0x00111ee0`, recursion `./kit fn 0x00111a00`, bounds
`./kit fn 0x0010fbf0`, chooser `./kit fn 0x0010feb0`, partition `./kit fn 0x0010f480`).
Each plant is a group part (`./kit fn 0x00129490`) at cell × the eco's cell size (+0xd4 =
extent/65536, 0.10546875 on free09). Free09 has 11 species: 9 trees with one kind-1 solid each and two
bushes with none. The game holds file species slot n at index n − 1 (and a plant's species byte too).

Which searches see the trunks: the bike's eleven body rays search the placed groups (their flags carry
the group bit); the sweep probe does not; the put-back clear uses S75's group rule (a part within
400 ft² flat that its reach puts closer than the probe's radius turns the spot away).

## How it was proved

- Function oracle, `crates/game-sim/tests/eco.rs`: `the_tree_builder_is_the_engines` (600 made-up
  meshes in the gate, a fifth of them trunks, the rest random soups with degenerate triangles) and
  `the_tree_builder_is_the_engines_ten_thousand_times` (`--ignored`, run once by hand: 10,000 meshes,
  2026 of them trunks, all bit-exact, 621 s). Each compares every node, facet and vertex bit for bit
  with the original's builder run in the Ee interpreter.
- Save state: `free09s_trunks_are_the_ones_the_original_built` reads the recording2 state's RAM: the
  9 trunks the game built (nodes, facets, vertices) equal ours from the disc's .esb and the RAM's own
  curve table; every plant's cell, species and width equal the RAM's; the cell size is 0.10546875.
- `a_free09_trunk_turns_a_put_back_away_within_twenty_feet`: one TreeB; a put-back 19 ft away is
  crowded, 21 ft away is not.
- `riding_into_a_free09_tree_stops_the_bike_at_its_trunk`: race125p on the Flat kit, full gas at one
  TreeB at z = 120 (trunk face 119.918 − 1.5): the bike falls from >50 to <10 ft/s at z ≈ 115.2 and
  does not wreck; on the open floor it never stops and is 460.6 ft along after 180 ticks.

## Not verified / not 1:1

- **The stop is not compared with the original's.** The task asked to break in the original riding
  into a free09 tree; I did not drive PCSX2 into a tree (no pad here, and a teleport + generated tape
  run was out of the session's budget). The test proves the port stops at the trunk, not where.
- **The port's bike sees trunks through its own probes.** The original's body rays (the ones that
  search groups) are not ported (S86/S87), so `World::groups` is swept with the port's contact bundles
  in `contacts`/`hits`/`hits_among`. Which bundle hits a trunk first, and so the contact normal and
  the stop tick, may differ from the original.
- After stopping, the port's bike under gas slides round the trunk and rides on. Not checked.
- No wreck on a trunk: the port's head ray never reaches it. The original's wreck test also only uses
  its head ray, so this is plausible but unconfirmed.
- `Ecosystem::stand` (plant height on the ground) is not proved against the original's arithmetic;
  the free09 cells compared are the file's, which the game keeps.
- `Trees::crowded` builds its list per call; the original's gather cache (refreshed less often) is
  not modelled.
- Solid kinds 0 (mesh file), 2 and 3 are not ported; free09 has none.
- The per-plant bend/sway state block (states < 2 and 4, a bend angle) is not ported; trees do not
  sway when hit.
- Group part flag bit 2 (keep the mesh where it is) is ignored; never set on free09.
- Other tracks: the cell size = extent/65536 is the loader's code, but only free09 was checked.
- The wreck bike, the biplane and the camera do not see trees in the port.
- The shell (`crates/openmx`) does not load trees yet; only the sim and its tests have them.

## Next

Follow-ups filed with `./kit plan add`.
