# 1 — quads touch (FINAL)

Task MD98a. 2026-11-18.

## What I did

- The held side was already general: since MD246, `atv_contact::Held::refill_among` holds a contact
  naming a known record id with that record's collider and body (`Knocked`), and `Held::resolve`
  answers it as `Other` with `Side::Body`. Another quad's record is that, kind 0 and kind 1 alike.
  So `atv_contact.rs` only gained docs saying so; `Held::refill` (no others) still refuses.
- `quad.rs`: the record's pass now takes a real `me`:
  - `Ride::step_quads(c, dt, world, segments, me, quads)`: a step among other quads.
  - `Ride::struck(me, mesh)`: this quad as another quad's pass strikes it (hull mesh at its scene
    node, its record, its collider and body as a `Knockable`).
  - `Ride::take_pushed(k)`: the body and collider another quad's resolution left.
  - Inside, `record_among` became a wrapper over `record_as(.., me, ..)`, and `frame_among` /
    `Knockable::step_own` carry `me` (the old paths pass 1, as before).
  - `record_on` itself is unchanged: it has no other quads; with none, `record_as` would do the same.
- Took nothing from ../opentennis (tennis has no vehicle-vs-vehicle contacts).

## What the original does

- C38's touch frame (research/oracle.txt `# C38`): records 0x01a511b0 and 0x01aa5020 each hold four
  entries naming the other, kinds 0,1,1,1 and 1,0,0,0. The gravity's call (`./kit fn atv_record_resolve`,
  the record's vt +0x88, handed 1) resolves them as MD75e1 found, with the other quad's body as the
  other side.
- Both records: collidable class 0x3012, collides with everything, actor bit 0x1000, damping off — the
  same words the port's `rig` lays (RECORD_CLASS / RECORD_COLLIDES / RECORD_ACTORS).
- The spring pushes only its own side. The other quad is pushed only by a hard hit's impulse (closing
  past 10 ft/s) and the friction.

## How it was proved

- `tests/atv_quad/quads_touch.rs` (`two_quads_records_resolve_their_touch_as_the_originals`), on
  `context/mxvatv-ps2/c38/c38-37523.ee.bin`:
  - each record's list read as the pass filed it, refilled with the other quad's record and body
    (`refill_among`), resolved by the port;
  - the original's vt +0x88 run in the interpreter on the same RAM;
  - record 0x01a511b0 first, then 0x01aa5020 on what the first left (the port carries its own
    pushed record and body into the second call);
  - after each: every word the port writes (both records' centre in the world, normal speed, relative
    velocity, the caller's entries' arms and relative velocities, both bodies whole) equals the
    original's, and the original wrote nothing else outside its stack. Checked it runs to the end
    (not a skip).
- `tests/atv_quad/mxu.rs` (`two_quads_meet_on_free09`): two quads from their files, one driving into
  the other standing across free09's track. Both records hold hits naming the other (126 and 130
  frames), the gravity's call answers them, and the struck quad was pushed by a hard hit (2 frames).
  A riding check, not a proof.

## Not verified / not 1:1

- The proof re-runs the gravity's call on the end-of-frame image (37523 is saved at vsync, after that
  frame's substeps). It is the original's function on the original's real touch state, not that frame's
  own first call. No image between the pass and the first substep was taken.
- Only one call per record. A whole frame of substeps among two quads is not compared with a capture.
- The pass's hits on another quad's hull are not proved: the port strikes the hull with Untamed's ray
  probe (`RecordAmong`) and the quad's wreck category word, neither checked against SLUS-21104 on a
  vehicle collidable. Filed as S481.
- Frame order is not the original's: each quad steps its whole frame in turn, and `take_pushed` drops
  the mirrors (kind 1) one quad's pass filed on the other's record. In the original all clears run, then
  all passes, then the world step, so those mirrors stay in the other's list. Filed as S481.
- The shell does not use `step_quads` yet: two seats' quads still ride through each other. Filed as
  N29 (vehicle.rs is not this task's file).

## Next

- S481 (order and the pass's hull hits), N29 (shell wiring).
