# 1 — quad bale resolve (FINAL)

Task MD246. 2026-11-18.

## What I did

- `atv_contact::Held` gets `bodies: Vec<Knocked>`. Each knockable body the quad's own hits name is held there:
  its collision record's resolution fields, its rigid body, the body's mass and its centre of mass.
  `Held::refill_among(filed, bodies, ids)` sets an entry's other to the body whose record id matches the
  contact's other word. `refill` is `refill_among` with none. `file_mirrors` puts the bikes' indices after
  the bodies.
- `Held::resolve` hands the bodies to `atv_contact::resolve` as `Side::Body(Weighed)` Others, ahead of the
  bikes (`Side::Vehicle`). The gravity's call per substep is unchanged; it resolves whatever is held.
- `quad::Knockable` gets `body: Option<Knocked>` (`with_body`). `record_among` holds the knockables that
  have one. `frame_among` hands each body back to its knockable after the frame's last substep, so a
  pushed bale carries its push into the next tick.
- Snapshot: `bodies` is skipped like `bikes` (both refill each frame from the knockables).
- tests/untamed_race_quad.rs: the bale knockable now carries its body read from the spawn image.
  New test `untamed_quad_resolves_its_hits_on_the_bale_as_the_original`.

## What the original does

- Untamed's quad record resolution is the record's vt `+0x88` (`./kit dis -p game 0x00206170`, on a scratch
  config with context/untamed and GAME_REL.BIN laid at 0x48ea80). It is SLUS-21104's 0x00205730 with the
  offsets moved (research/oracle.txt `# MD246`):
  - entry kind is an lh at +0; surface an lh at +2; other record +0x80; mover +0x84;
  - collidable flags at +0xc0;
  - its callees sit at the same call sites as SLUS-21104's (paired by call order).
- So the port is MX Unleashed's resolution on the ATV layout (`atv_contact::resolve`), the other side the
  bale's rigid body (record +0xe4, mass +0x18, centre of mass +0x1a0) instead of a vehicle.
- Tick 197 of research/md224 steer: two hits of the quad's record on the bale's top, both naming the bale
  record 0xaac3d0. The quad leaves the bale at 22.48 ft/s along the normal, past the spring's reach, so the
  quad's resolution pushes neither body that tick.

## How it was proved

- Function oracle: Untamed's 0x00206170 run in the interpreter on MD147's spawn image against the port's
  `Held::resolve`, 10,000 cases (case 0 the real tick-197 inputs from the port's own pass; the rest
  perturbed bodies, normals, arms, speeds). Compared: both bodies' words the resolution writes, the
  record's words, each entry's arms and relative velocity, and no stray writes outside the stack. 758
  cases took the impulse, 9,242 the spring. Bit-exact.
- The port's pass for tick 197 (`record_among`) holds 2 entries, both naming the bale, with 1 body.
- The whole tick 197 (`step_among`): the gravity's call is answered on every substep, and the bale's
  body comes back unchanged, as the image's.
- Mutations: holding no body fails the test; stepping with the bale's body removed fails the gravity
  assert.
- The other 5 tests in untamed_race_quad.rs still pass.

## Not verified / not 1:1

- No capture row holds the state just after tick 197: row 197 is a late read (row 196's body), and row 198
  comes after tick 198's whole step, including the bale's own resolution (MD247). So the bodies after
  tick 197 are proved by the oracle and by "nothing pushed", not against a capture row.
- Row 198 (the bale's shove) is not reproduced: it needs the bale's own record resolving its mirrors
  (MD247).
- The callees (relative, spring, impulse, friction, damping) were paired by call order, not diffed. The
  oracle covers them on the cases it ran.
- The oracle's cases are perturbations around tick 197's real case, not a sweep over all of the
  resolution's inputs.
- The other body's centre of mass is read at body +0x1a0, the same as the quad's. Only checked through the
  oracle on the bale.
- The bale's body +0x0 word (it flips at row 197) and body +0xe8 (0x20000 on both bodies) are not modelled.
  Their meaning is unknown.
- The quad's record built from its files differs from the image's in the inverse inertia (record +0x70)
  by ~36 ulps. The test makes the quad at the settled spot heading +z, not at the start spot the part
  reader used (MD134). The test excepts that field, and the oracle reads the image's. Filed as MD250.

## Next

Done; MD247 (the bale's own resolution) and MD250 (the record's inertia) follow.
