# 1 — shared record glue (FINAL)

Task MD141. 2026-11-17.

## What I did

- `tests/atv_quad/record_outside.rs` is now the one record-glue module for both tests. It is
  self-contained (its own imports and word helpers), so each test mounts it by `#[path]` and globs it in.
  It also mounts `gather_in_ram`/`heap_in_ram` and re-exports them, so the bike test gets one copy of each type.
- `tests/mxvatv_bike_ride.rs` lost its copies of the moved code (≈1330 lines). It mounts the module where
  it used to mount `gather_in_ram`.
- Item-by-item diff of the two copies first (script, not kept): most items were the same text. The bike
  had moved on, and the shared module takes the bike's newer code:
  - K73's `HashSet` in both searches.
  - K64's stubbed-allocator path in the pair log push.
  - F49's kind 6 box pair (`oriented_box`, with `f53_box_in_ram` moved in).
  - F60's lent database (`LentInRam`, `sweep_walk_written`, `write_lent`, `lend`/`take_back`).
- MD118's two fixes now cover the bike's glue too:
  - The mesh pair's tree-walk setup globals, with MD142's `WALK_SPOIL`.
  - A plant mesh stood by `atv_sync::stand`. `plant_stand_with` keeps `frame_at`: `Some` still calls the
    part's own slot, because F54's mesh oracle stubs that slot on a made-up vtable. `None` is MD118's port.
    The quad image's `image::collidable` mesh branch is mirrored as `mesh_collidable_in_ram`, since the
    bike crate has no `image` module.
- The quad gains the bike's kind 6 box pair: before, it panicked on kind 6.

## What the original does

No new behaviour: this task only moves test glue.

## How it was proved

- `./kit check -- -p game-sim --test mxvatv_bike_ride`: all 149 pass. Run by hand with output on:
  - ride/flip replays with the part search and the sweep in Rust.
  - F51 nearby/global sweeps (10,000 cases each).
  - F54 plant stand on a sphere and on a mesh.
- `./kit check -- -p game-sim --test atv_quad`: 122 pass, including sim_frame's palm (MD118/MD142's RAM
  comparison and store counts). The one failure is `mxu::the_quad_rigged_from_its_files_is_md123as`.
  It fails the same way with this task's changes reverted, because MD134 changed the rig. Filed as MD245 (critical).

## Not verified / not 1:1

- The bike's tests never reach MD118's two fixes. Its part searches pair kind 5 only (no mesh, so no
  tree-walk setup), and its plant stands in searches are spheres. Both fixes are proved only by the quad's
  sim_frame palm, as in MD118/MD142.
- `mesh_collidable_in_ram` copies `atv_quad/image.rs`'s mesh read instead of sharing it, because the bike
  crate has no `image` module. The two copies can drift.
- The quad's new kind 6 path (`oriented_box`) is proved by the bike's F53/F51 oracles. No quad ride meets
  a kind 6.
- The gate's `atv_quad` binary is red on main from MD245 (MD134's rig), not from this task.

## Next

None.
