# 1 — atv wreck body (FINAL)

Task MD65a. 2026-11-03.

## The PC first: `pc_quad_init`'s branches

`./kit fn 5d6750 -o pc` (MXVehicleAtv::vf84). Its branches:

- Each `operator_new` result is checked for null (7 allocations): a null part is left 0 and its
  constructor is skipped.
- Each part's scene-tree address is `part + 0x24`, or 0 for a null part (two checks).
- The tail's `FUN_00477fe0(...) != 0` calls vt +0x30.

Nothing else branches. The block the brief is about (from the second `operator_new(400)` to the
`TRIGGER` category) is straight-line code. The order of its calls matches the PS2's call for call.

## What SLUS-21104 does (`./kit dis 4b9db0 4ba310 -p atv`)

The full field map is in research/oracle.txt under `# MD65a`.

- **Two parts, not one.** +0x1430 is a ray probe with no file. It gets flags word 2, the quad as its
  owner, a place in the scene tree, and the category list [TRIGGER, the quad's +0x30c].
- **The wreck rays.** +0x1434 is the wreck rays:
  - flags word 3;
  - loaded from `WreckATVm.col` on the scene node (+0xe4), then readied (vt +0xc0);
  - category PHYSICSBODY (0x1000, the sweep's too);
  - owner the quad (+0x88), record the quad's collision record (+0x90 = +0xc8);
  - hung in the scene tree.

  +0x1438 keeps the riding rays (+0xcc) for the swap back.
- **Lists.**
  - The riding rays and the sweep add both new parts to their ignore lists.
  - The wreck rays ignore the trigger probe, the riding rays, the frame mesh and the sweep.
  - The trigger probe ignores the riding rays, the frame, the sweep and the wreck rays.
  - The frame mesh's list is not changed.

  The list adds (`atv_ignore_object`, `atv_ignore_category`) are the same as MX Unleashed's: return if
  the entry is already listed, else take the first 0 slot, else grow the list. The object add skips 0;
  the category add does not.
- **The constructor.** `0x001bb0b0` → `0x00113ee0` leaves all three list modes at 1. That is why the
  wreck rays' category list is in mode 1 although the block never sets it. The spawn image showed this,
  and the port's `Col::made` does the same.
- **The flags word.** It sets flag-byte bits 3/1/2 from its own bits 8/2/4. Words 2 and 3 give the same
  byte.
- **What the wreck does with it.** The quad's vtable has the bike's two swaps at +0x48c and +0x490
  (checked in the image):
  - `atv_bike_wreck_entry` calls +0x48c (0x004e232c), and the unwreck `0x004e2380` calls +0x490 (0x004e2404).
  - Each swap writes the target rays into quad +0xcc, quad +0xd8 and record +0x200.
  - It hides the other rays' scene child and takes them off their world.
  - Then, only if the target has no world (+0x20 is 0), it shows the target and puts it in the quad's
    world (+0xb4, class +0xc0).

  So while the quad is wrecked, its collision record is filled by the `WreckATVm.col` rays, not the
  riding rays.

## Checked

- `./kit check -- -p game-sim --test atv_quad`: TESTS PASSED, 39 tests. The three new tests also ran
  through cargo directly, so I saw them run rather than skip.
- `the_quads_wreck_body_is_built_as_the_original_builds_it`:
  - Runs the original block from 0x004ba03c to 0x004ba230 with s0 = the quad, on 10,000 generated cases.
  - Inputs: the three existing collidables' lists (any length 0–5, 0 slots, entries that are already the
    new parts), the actor word (0, equal to TRIGGER's word, or another), and the category answers.
  - Hooked:
    - the allocator, which hands out two fixed parts;
    - the constructor, which writes its vtables and list modes;
    - flags, scene add, load, ready, the category lookup;
    - the realloc, which copies and bumps.
  - The list adds and list modes are not hooked. They run as they are.
  - Compared:
    - all five collidables' object and category lists;
    - +0x1430, +0x1434, +0x1438;
    - the wreck part's +0x88, +0x90 and +0x94;
    - the call list and the names asked.
  - Nothing outside the collidables, the stack and the scene's dirty byte is written.
  - All 9 paths were seen ≥ 100 times: "already listed" in each list, empty-slot reuse, growth, and
    each actor case.
- `the_spawned_quads_wreck_body_is_the_ports`: the spawn quad in `atv-r-spawn.ee.bin`, built by the
  game itself.
  - The image's three lists, with the two parts taken out, go into the port. All five lists come back
    exactly as the image holds them.
  - The image's own `atv_category_of`, run on the real strings, answers PHYSICSBODY 0x1000 and
    TRIGGER 0x8000 without writing.
  - The ownership words match.
  - The quad rides on its riding rays: +0xcc, +0xd8 and record +0x200 all equal +0x1438, the riding
    rays are in a world and the wreck rays are not.
- `the_quads_ray_swaps_are_the_originals`: both swaps on 10,000 cases. The three words start as
  garbage and either target may or may not be in a world. The three words and the calls (hide,
  detach, show, in-world, each with its arguments) match. Every path was seen ≥ 1000 times.
- Mutations turned the tests red:
  - the category add given a 0 guard
  - the wreck rays not ignoring the frame
  - `in_world` inverted
  - made parts in mode 0
  - no empty-slot reuse
  - the record word not swapped
  - the two category adds swapped
- `./kit audit`: only the existing plan/DONE.md:519/:533.

## Not verified / not 1:1

- **Allocation failure is not ported.** On the PS2 a null part would mean writes through address 0
  (+0x88). The port takes the handles the allocator gave and never generates a null.
- **The part's own bodies are hooked, not ported:** the constructor, the flag byte, the load of
  `WreckATVm.col` (its rays and reach), the ready virtual, the scene-tree add, hide/show, the detach
  and the world add. The port records them as `Call`s.
  - The flag byte is described above but not modelled.
  - The wreck rays' ray probe can be built from the file like `bike_col::BikeCol` (MX Unleashed's
    `m.col` reader). I did not check that the file loads that way.
- **No wiring.** Nothing calls `build_wreck_body` or `swap_rays` in the ride yet, and the quad's wreck
  entry is not ported (the bike's, `atv_bike_wreck_entry`, is hooked in the other tests). There is no
  capture of a wrecked quad. Which rays a wrecked quad's record shows was not watched live.
- **Two words have no traced reader:** what reads quad +0xd8 (written with +0xcc), and what the trigger
  probe is for (TRIGGER category).
- **The rest of `atv_quad_init` is not ported:** the wheels, engine, vehicle base init and tail. The
  brief asked only for the wreck body.
