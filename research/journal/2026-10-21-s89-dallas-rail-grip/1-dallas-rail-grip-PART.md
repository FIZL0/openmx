# 1 — dallas rail grip (PART)

Task S89. 2026-10-21.

## What I did

- Read free06's scene, its collision meshes and its terrain off the disc (`tests/dallas_rails.rs`), and
  rode the port on the railway in a test (`tests/dallas_rails_port.rs`).
- **What the railway is to the collision.** It has no rails or sleepers. It is a raised ballast bed made
  of 62 of the scene's collision meshes, `Free06_RR_COL_01..62.col`:
  - v2 meshes, 50 triangles each (40 for `_62`), all placed at (3452.57, 135.555, 3463.21) facing +z.
  - Each is a trapezoid ridge about 24 wide. Its flat top is about 14 wide and stands up to 0.94 above
    the terrain (the test asserts 0.8–1.0). The side slopes are about 13° (normal ≈ (0.229, 0.973)).
  - **Surface word 1** on every mesh.
  - They are props of their own, not part of the terrain mesh. The terrain under them is surface 0
    (1893 of the beds' vertices) or 12 (333). The whole terrain is {0, 12, 15}.
  - `Free06_Rail.col` is something else: three upright boxes, 24×3×0.22, surface 7, near (4000, 276,
    3180). They look like fences or buffers, not the track.
- **Material and friction.** In race125p.phy every surface row has MaxSideForce 5 and SkidSideForce
  4.85.
  - Front SideForce is 0.165 on rows 1–4 and 9–10, 0.12 on rows 5, 7 and 8, and 0.07 on row 6.
  - Rear MaxDriveForce is 2.2 on rows 1 and 9–16, and 2.0 on rows 2–8.
  - Nothing in the table makes a slide. In any case the original's tire never reads the bed's surface
    (see below).
- **What the port gave the bed: the slide.**
  - The bed reached the ride only as a prop, through the ride's collision bundle (`ride.rs` `bundle`,
    which is the port's own composition).
  - The bundle's fourth probe goes straight down from the centre of mass and reached
    `probe_length + radius`. That is deeper than either wheel's suspension line ends.
  - On the bed's flat top it hit first (near ≈ 0.996) and held the bike up where neither line reached.
    Both wheels had neither a ray hit nor a swept contact, so there was no tire force.
  - Gas gave 0 forward speed while the sideways speed crept from −0.03 to −0.36 and kept growing. On
    the shoulder the bike slid at 2.2–3.6 ft/s until it fell off the bed.
  - Terrain only, from the same spot, it rode normally.
- **Fix (stand-in, the port's own): `ride::belly_reach`.**
  - The straight-down probe now stops a wheel radius short of the shallower suspension line's end,
    measured along the chassis' down, with a 0.05 floor. On anything flat the wheels meet the prop first,
    and the belly only catches once the suspension is a radius into its travel.
  - Riding along the top with gas, the bike now reaches 88.07 ft/s at frame 135 against 88.14 on bare
    terrain. The sideways speed is about 0 and the body probe never touches.
  - Along the shoulder it reaches 87.6, with at most 0.87 sideways while it settles.
- **Fix (1:1): the wheel's surface byte.**
  - Before, the port wrote `wh.surface` only on a ray hit, from `World::surface` at the contact point,
    using its own nearest-corner rule (`tx > 0.5`).
  - The original's heightfield query writes it on every pass of the ray, at the probe end, hit or miss.
    It is written from the terrain alone, by the nearest corner by squared distance.
  - The port now does the same: `contact_search::nearest_surface`, `World::wheel_surface` (overridden
    by `Ground` and `Place`), and the call after `wheel::suspension` in `ride.rs`, guarded by
    `has_ground` as the original's loop is.

## What the original does

- `./kit fn 0x1dc960` is the heightfield query. Its fifth argument is the wheel's surface byte, wheel
  `+0x110`.
  - It clamps `x*inv` and `z*inv` at 0 and truncates to a cell (even cells when coarse).
  - If the cell cannot be read, it returns before writing anything.
  - Otherwise it computes `u` and `v` in the cell, then the four squared distances in this order: the
    sample, +x, +z, both. It takes the first minimum (`c.le` keeps the old one) and writes that corner's
    surface byte.
- `./kit fn 0x25b280` is the wheel's suspension. It runs the ray loop (up to five passes until the
  normals agree) and calls the query once per pass with the probe end and `wheel+0x110`. So the byte
  holds the terrain's surface at the last probe end, whether the ray reached the ground or not.
- `./kit fn 0x25b250` is the swept contact's setter. It never touches `+0x110`. So a wheel riding the
  railway bed reads the terrain's 0 or 12 under it, never the bed's 1.
- `./kit fn 0x25c390` (surface drag) indexes the tables at `+0x778`, `+0x7b8` and `+0x7f8` by that byte,
  on the ground or not.
- The original holds the bike up on a prop with eleven body rays from the bike's `%sm.col` and
  `contact::resolve_ridden`, not with the port's four-probe bundle. Porting that is S86/S87.

## How it was proved

- **The bed's shape and surfaces:** read off the disc in
  `dallas_rails::the_railway_is_a_raised_bed_of_its_own_meshes_over_the_terrain`. It asserts 62 beds,
  surface 1 on each, a rise of 0.8–1.0, the terrain surfaces {0, 12} under the beds, and that
  `Place::wheel_surface` at every bed vertex is the terrain's corner.
  - I proved it runs by planting a wrong count (63) for one run; it saw 62.
- **The corner rule:** `dallas_rails::the_wheel_reads_the_nearest_corner_and_nothing_off_the_terrain`,
  on a synthetic one-cell field. It covers the four corners, both kinds of tie, the clamp below zero, and
  `None` off the field. The rule itself comes from reading 0x1dc960; it was not run against the original
  (see below).
- **The ride:** `dallas_rails_port::the_wheels_carry_the_bike_along_the_railway_bed`. It rides the top
  and the shoulder with and without props and asserts:
  - the body probe never touched after the first second;
  - at most 6 wheel-frames without contact;
  - sideways speed under 1;
  - forward speed within 5% of bare terrain.
  - With the old reach, the same ride sat on its belly every frame. I proved the test runs by planting
    `belly == 1` for one run; it saw 0.
- `./kit check`: passed.

## Not verified / not 1:1

- **The acceptance is not met.** No ride on the rails was captured in the original or replayed
  bit-exact. No capture of any kind replays bit-exact past tick 1 today. That needs:
  - the original's wheel start state from a save state (S64, still open; S58 is blocked on it);
  - the original's body rays and contact resolution in place of the port's bundle (S86/S87).
  
  Recording rides now would give captures with nothing that can match them, so I did not drive the
  emulator.
- **`belly_reach` is the port's own** and is not taken from the original. Neither the radius margin nor
  the 0.05 floor is the original's. It changes every ride on a prop where the body probe used to reach,
  e.g. landing flat on a ramp's deck, where the wheels now take the hit before the belly. It goes away
  when S86/S87 replace the bundle.
- **The bike is still sliding on the bed, not gripping.** The wheels ride the bed as swept contacts, so
  their slip comes from `wheel::tire_forces` on a prop contact as the port composes it. Nobody has
  compared that slip with the original's on the bed.
- **`nearest_surface`** follows 0x1dc960 as read, in ps2-float, but I did not compare it against the
  running original. The surface tables it indexes, and `Ground::at`'s own height blend for the ray, are
  unchanged and still the port's own.
- **Worlds without a terrain** (`Flat` and the test worlds) still answer the wheel with
  `World::surface`, now on every pass instead of only on a hit. For a world with one surface this
  changes nothing.

## Next

- After S64 and S87 land: make a free06 save state on the bed, capture rides along and across the track
  with `./kit record`, replay them through the contact, and compare the wheels' slip. Then remove
  `belly_reach` along with the rest of the port's bundle.
