# 1 — atv on mxu (FINAL)

Task MD133a. 2026-11-13.

## What I did

- `quad::Ride::build_wreck` now answers the wreck body's part calls itself instead of handing them to
  `rest` (`WreckLoad`, crates/game-sim/src/quad.rs). MD65a's `build_wreck_body` is unchanged:
  - `PartMade`: the part's `Made` (made, flag byte `PART_MADE_FLAGS` 0x06, the constructor's bits 1|2) and
    its box state (`Collidables::states` slot) cleared;
  - `PartFlags`: `quad::part_flags`, the flags call disassembled (word bits 8/2/4 → byte bits 3/1/2);
  - `PartHung`: hung under the quad's node;
  - `PartLoad(Wreck)`: the file is asked for by name (`WRECK_FILE`) through a reader (`quad::ReadRays`)
    the way the init's load asks for it. It fills the wreck rays and the part's box kind (the file's tree
    root box and its first segment, as a ray probe's box reads them);
  - `PartReady(Wreck)`: the part's vt +0xc0, which disassembles to a tail call of its own vt +0xc8
    (the node's frame, then the stand). So the wreck rays are stood with `atv_sync::stand` at the quad's
    scene node's frame, with the node brought up to date first.
- `quad::WreckParts` (the five lists, both `Made`s, the trigger segments) is kept in the new field
  `Recorded::parts`.
- `Ride::shape_trigger(tires)`: the trigger probe's segments (`trigger_segments`) are set once and the
  probe is put on the node. In the original, vt +0x474 has one call site on the quad: the node attach
  (quad vt +0x470), as that function's last call. `Ride::trigger_segments()` hands them out.
- `Ride::rig_on(load, riding, read)`: the rig with the wreck file coming from the loader. `Ride::rig` keeps
  its signature and answers `WRECK_FILE` with the file it is given. The shell (`crates/openmx/src/vehicle.rs`
  `AtvRide::made`) takes its sweep segments from `ride.trigger_segments()` rather than working them out
  itself.
- research/oracle.txt "MD133a": the disassembly and the image's words.

## What the original does

See oracle.txt "MD133a". Flags: three byte writes, then a global byte zeroed and a no-op call on the
scene child. Ready: vt +0xc8. The trigger's shape (`0x001bb3f0`) rebuilds the probe's ray tree from the
four segments, allocating as it goes, and sets its node.

## How it was proved

Against MD37a's spawn image (`atv-r-spawn.ee.bin`) and MXVSATV.PSR's WreckATVm.col, in tests/atv_quad/mxu.rs:

- `the_quads_wreck_parts_are_the_images`: both parts are made, hung under the quad's node (the image's
  scene child +0x24 has the parent quad+0xc) and on its scene node (+0x174 = quad +0xe4), and owned as in
  the image. The wreck rays' flag byte equals the image's whole (0x02). The trigger's matches in the
  flags call's bits (0x0e). The wreck rays' tree box and bound from the file equal the image part's bit
  for bit. Stood at the frame the image's part was stood at (+0x130), their centre and reach equal +0x50
  and +0x70 bit for bit.
- `the_quads_trigger_and_wreck_file_come_from_its_loader`: the quad made from its files asks its loader
  for `WreckATVm.col` once, by that name. Its four trigger segments equal the rays of C28's spawned quad's
  trigger probe bit for bit, compared as a set. The probe stores its rays in its tree's order (ray 0 is
  the right front wheel's, ray 1 the left front's, and the rears the same way), not the wheels' order; the
  wheel mounts themselves match C28's wheel objects (+0x17b0.. +0x90) bit for bit. With no file, the parts
  are still made and hung, and the wreck rays are empty.

`./kit check -- -p game-sim --test atv_quad -- mxu`: 11 tests pass (MD133's 9 and these 2). The image and
both `.col` files were there, so neither test skipped: the failing runs along the way printed the image's
words.

crates/game-render/src/quad.rs (on the task's line) needed nothing: it maps the chassis nodes for drawing,
and the wreck body draws nothing.

## Not verified / not 1:1

- **The trigger probe's tree** is not built. `0x001bb3f0` rebuilds the probe's ray tree from its segments
  and allocates while doing so, and that tree builder is not ported. On an MX Unleashed world, the sweep
  (`OnWorld`) takes the four segments directly, and nothing reads the trigger's box (there is no spatial
  database), so its box state stays the default. Its stand in `Call::Contacts` therefore gives a zero
  centre and reach where the original's reads its tree (the image's reach is 2.757).
- **The trigger's flag bit 4 (0x10)**: the image has 0x12, the port 0x02. Neither the flags call nor the
  shape writes bit 4. Of +0x126's 62 byte writers, I did not find the one that sets it, so it is probably
  a runtime bit. Only bits 1–3 are compared.
- **The flags call's other effects**: a global byte (0x003dd300) zeroed, and a call on the scene child
  that returns at once. Neither is kept.
- **Hanging is a flag**: the scene child's sibling links and its visibility bits (+0x21) are not modelled.
  The port's scene holds frame nodes only.
- **The swap's "in a world already"** test still passes `true` (MD123a's): the parts' world membership
  (+0x20) is not tracked, so the first swap to the wreck rays does not hand on the show and in-world calls
  the original makes (the wreck rays' +0x20 is 0 at spawn). On an MX Unleashed world both calls go to
  `rest` and do nothing.
- **The readied frame on a files-made quad** is the spawn frame at rig time. The original's init stands
  the wreck rays at the node's frame before the quad is placed (the image's wreck +0x130 is about 18 ft
  from where the quad sits). Only the stand's arithmetic is compared, at the image's own frame. On an
  MX Unleashed world the collision sync is off (`Ride::spawn`), so nothing reads that box.
- **The trigger's stored ray order**: the port keeps the segments in wheel order, which the sweep needs
  to give each hit to its wheel, as `OnWorld` did before. The original's probe holds them in tree order.
  The segments are proved as a set, and their wheel order by MD65b's oracle of vt +0x474.
