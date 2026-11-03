# The quad's wheel integrate, whole: surface drag and spin (MD75)

2026-11-03. Task: `./kit plan MD75`.

## What was ported

`atv_wheel_surface_drag` (0x004a1ec0) and `atv_wheel_spin_integrate` (0x004a2040) are the last two calls
the quad's Force body makes into code with no port. They are now `atv_wheel::surface_drag` and
`atv_wheel::spin_integrate` (with `contact_frame`, `force_law` and `gyro_torque`). The field map is in
research/oracle.txt under `# MD75 (part 5)`. They are MX Unleashed's `tire::surface_drag` and
`tire::spin_integrate`, moved to SLUS-21104's offsets and its rigid body. The differences:

- the drag goes in as a local force through body vt +0x74;
- the hinge takes a quarter of the force (0.25 at 0x00525230);
- a wheel off the ground whose partner is down borrows the partner's load, scaled by +0xb8, and its ground.

## Proof

- `tests/atv_wheel.rs` (commit 012f4c8f) runs each one alone against the original:
  - surface drag over 10,000 states, every path at least 500 times;
  - spin integrate over 20,000 states, all 28 paths at least 100 times;
  - five mutations went red (hinge scale, drag table, angle wrap, axis row, cleared).
- `tests/atv_quad.rs` gives each wheel a `Spin` (`Bike::spins`) and pokes its words. That includes the
  brake pointer at the quad's front or rear brake and the partner pointer, as the image has them.
  `MORE_REGIONS` now covers the contact frame. `the_quads_wheel_spin_is_the_originals` runs the Force body
  10,000 times with the wheel integrate, surface drag and spin integrate unhooked. Each wheel's path is
  one of: neither down / borrows the partner's load / into the hinge / into the body. Each path is seen
  at least 100 times, and every region is equal.
- Two more mutations went red: the partner table, and the front/rear brake choice.
- All 50 atv_quad tests pass.

## Decisions

- **The steps run the two calls inline, so a test that unhooks the wheel integrate unhooks them too.**
  `Steps::call(WheelIntegrate)` runs `Steps::call(SurfaceDrag)` and `Steps::call(SpinIntegrate)` itself.
  A `Moto` decides only at the top level, so the steps cannot leave a nested ported call to the original.
  So `the_quads_wheel_integrate_is_the_wheels`, `…force_body_runs_the_bikes_steps` and
  `…force_body_runs_every_ported_step` now list SURFACE_DRAG and SPIN_INTEGRATE in their picks. A test of
  the drag alone under the original's wheel integrate cannot be written this way. atv_wheel.rs already
  has that test.
- **`Spin` overlaps other words, and those stay where they were.** The spin, torque, spinning flag and
  forward force belong to `Parts::drive`. The rear pair, donut and 1/radius belong to the quad's `Wheel`.
  `spin_in` and `spin_out` copy them across, and the test pokes the `Spin` words first so the owners' words
  win.
- **Ground records are kept under 16 in the generator**, because they index the sixteen tire tables. A
  record of 16 or more would make the original read past the tables. Real levels never hand one back
  (MD65b's remap keeps surfaces under sixteen).

## What is still open

- The partner-borrow path is proved only on still ground in atv_quad. On objects and movers it is proved
  only in atv_wheel.rs.
- The brief's whole frame (one frame off atv-r-spawn.ee.bin with only atv_ground_query hooked) still needs:
  - the image→Ride reader;
  - the PostProcess calls that are not ported (spatial update, collision sync, node bounds);
  - the other listeners in the world;
  - the collision sweep (MD86).

  These are filed as follow-ups.
