# 1 — world moving (FINAL)

Task MD153. 2026-11-14.

The user, 2026-11-13: "in mx world trucks and props have no collision". On a world the tiles' path
cars, knockable props (S131) and animated toys (V43) were only in the picture: the shell loaded a
world's `Traffic` empty and left the props block to tracks.

## What I did

- **`game_sim::tiles::Tiled` has a moving list** (`moving`, handed out by `World::moving`), as a
  track's `Props` are. `Traffic::attach` pushes every car's and toy's mesh onto it as on a track.
  Car contacts read it from `first..`; `struck_among` matches cars by object id; `span`/`car_at`,
  ghosts' `hide`/`show` and `Trucks` all index it. So the near sets the bike's searches use
  (`wooded_near`) and the plane's set (`every_shape`) carry a **mirror of it at index 0..**
  (`mirror`): clones spliced in at the head when the count changes, else each prop's pose, bounds
  and `moving` synced every hand-out, and its mesh cloned again when hide/show changed its size.
  A prop's `heard` flag set on a mirror is forwarded to the master in `moving_list`. `sweep_here`
  sweeps the list as owner `MOVING_OWNER` (1 << 24, the port's own numbering).
- **The shell loads every tile's moving things** (`world_dress::moving`). For each tile it reads
  the `.toy` cars (`Traffic::sim_moved`, the cars' keys moved by the tile's offset in F adds) and
  their `HasPhysics` bodies, put back at the world's edge. It also reads the animated toys
  (`Animated::join` moves their places and prefixes their models `<tile>::toy:`) and the knockable
  props (`PropSet::load` over the tile's scene models moved, joined into one set by
  `PropSet::join`). A model outside the tile's keep gets no body, as the picture leaves it out
  (MD27). `Traffic::join` and `PropSet::join` are new and only append.
- **Every tile is loaded at ride start, near or not.** Both netplay seats load the same world, so
  they hold the same list with the same indices. Nothing streams in by distance.
- **The picture.** Cars are drawn as before, from each dressed tile's clips (MD15). Toys are drawn
  through the ride's `animated`, now the world's. A knocked body is found among its dressed tile's
  props (`world_dress::tile_at` by keep, then name and rest place, once). It is drawn at its node,
  blended between the last two ticks as V126 does: on the GPU by the new
  `Renderer::move_owned_prop(owner, k, at)`, and on the software path from `world_props_shown`.
- **Headless `props moved:` line** at a ride's end: every body a foot or more off its loaded node.
- A track ride is untouched. `Toy::sim` is `sim_with(None)`, the same arithmetic as before. The
  props block for a track is as it was. A track's `Props::moving` is unchanged.

opentennis had nothing to take for this (it has no worlds or moving props).

## What the original does

The original has no worlds. Each tile's cars, toys and props are the original's own for that map,
loaded as the map alone loads them (`.toy`, `.pco`). The world only moves them by the tile's offset.

## How it was proved

- `crates/openmx/tests/world_moving.rs` rides the shell headless on world_mxu_free (openmx-mods'
  stitch, from `$OPENMX_WORLDS` or the main checkout's `mods/`). Two rides:
  - **free06 train, full gas square at its side** (tests/train.rs's user's path moved by x 144):
    thrown (reason 1), and it ends 89 ft east of the line, not past it.
  - **free09 beach balls, full gas from 60 ft west:** two `BeachBall` bodies moved (21598,128,3538
    → 21765,130,3581 in one run).
- **Falsified:** with `Tiled`'s `World::moving` returning `None`, the bike standing on the rails at
  (2529.8, 3779.9) facing the train rode out 100 ticks with no wreck. With the list it wrecks
  (reason 1).
- A temporary print showed every knocked free09 body mapped to its drawn prop
  (`mxu_free09::BeachBall` k 300, 312–315, eggs, cans, barrels). It was removed before commit.
- The tiles, traffic, toys, prop_set, prop_near, prop_knock, head_on_props, physics_cars,
  physics_car_struck, kept_world, world_search and two_p_world tests passed (72). So did train.rs
  and world_moving.rs (6). `./kit audit` is clean.

## Not verified / not 1:1

- **The car keys are not bit-equal to a tile ridden alone.** They are moved into world feet by F
  adds, so the floats differ from the tile-local ones. On a world the cars are the world's ride,
  not the original's.
- **All tiles are loaded, not by Reach.** This is a deliberate netplay choice (both seats the same).
  It costs load time and memory, which were not measured beyond one debug run (first frame 6.3 s,
  545 MB peak, with 49 cars, 7 toys, 181 collections and 196 bodies).
- **No wrap (seam) copies of moving shapes.** On a wrapped world they are met only at their own
  copy. Filed as MD153a.
- **Woken `HasPhysics` cars are still drawn on their clips** in a world (`Dressed::cars_near`), not
  where the sim's body is. Filed as MD153a.
- **The world toys' drawing was not looked at on screen.** Only the load ("7 toys") and the matching
  spec/model counts were checked.
- **The knocked-prop picture was not looked at on screen.** It was checked only by the mapping
  print. The software path's draw of a moved prop was not run.
- **Shared toy textures.** Toy models are prefixed per tile, but their textures go through
  `Assets::load_as` as the ride's, not taken in per tile like the dressed props. Two tiles' toys with
  a same-named texture share one.
- **`MOVING_OWNER` numbering.** The value (1 << 24) for the moving list's sweep owner is the
  port's own.
- **No two-seat netplay run.** Determinism rests on both seats loading every tile in the world's
  order.

## Next

MD153a: woken cars drawn where the sim has them, and wrap copies of the moving list.
