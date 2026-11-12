# 1 — atv sweep pair hits (FINAL)

Task MD119. 2026-11-12.

## What I did

- `tests/atv_quad/sim_frame.rs`: MD105b's `OasisSweep` was its own `Narrow` that called MX Unleashed's
  `sweep_probe::pair`. It is now an `atv_sweep::Outside` (it only hands each prop's mesh over as a
  `Target`), and `WorldStep::sweep` searches with `atv_sweep::Sweep`, so the record's sweep goes through
  the ATV's own clear and pair (`atv_sweep::pair`). `WorldStep::sweep` returns a `PortSweep`: the bounds,
  the hit bit, the probe and the record's `More`, and each segment's struck collidable as the image's
  address.
- `RecordInRam::sweep` (Port pieces): on a hit, `sweep_record_is_the_originals` compares the probe and its
  record with RAM after the original's search. That covers the world start, direction and length; the
  centre and radius; the record's two head quads; the nearest normal, point and push; the tally and the
  nearest fraction; and segment 0's fraction, point, normal, hit word, owner word, `+0xac` word and
  struck collidable. Then the record pass gets the **port's** `Swept` (normal, point and struck from the
  port's record), so the put-out runs on the port's hit. Before this, the original's answer was used
  even with the port's pieces.
- A ride where the sweep hits: `palm_kicked`, the palm ride with the body's velocity (`+0x80` xyz)
  doubled before frame 87 on both machines. The record's sweep point is 3 ft over the body (offset
  `(0,3,0)`), and the sweep goes back two frames of velocity (lead 2.0, push-out 2.0, read off
  `atv-r-spawn.ee.bin`'s record). At about 160 ft/s that point crosses the palm's trunk before the
  part's rays push the quad off it. The original's sweep hits on frame 90. I found this by scanning
  frames {70, 78, 84, 87} × factors {2, 3, 4} on the original machine alone. Only (87, ×2) hit.
- New test `the_sweep_that_hits_the_palm_is_the_ports`: 130 frames with the Port pieces (record,
  outside and world step all the port's). It needs at least one hit and checks every RAM word outside
  the stack against the original's whole frame, every frame. The palm test
  (`the_palm_strikes_contacts_push_the_ports_quad`) now asserts its 300 sweeps all miss: 239 pairs and
  0 hits, measured.
- `WorldStep::asked` has a sixth counter (sweeps that hit), and the MD105 log line prints the counters.
- Nothing taken from opentennis: it has no sweep probe.

## What the original does

The record's sweep back sets one segment on the sphere probe, places it, and searches the nearby list.
The probe's pair is vt `+0xb4`, already ported and described in `atv_sweep::pair` (F51). On a hit the
record reads segment 0's normal, point and struck object off the record's arrays (`+0x94`, `+0x98`,
`+0x9c`). Then it puts the body out along the normal.

## How it was proved

Ride replay against the original in the interpreter (`tests/atv_quad/sim_frame.rs`,
`the_sweep_that_hits_the_palm_is_the_ports`). There are two machines. On one, the original's whole sim
frame runs. On the other, the frame runs in pieces, with the quad's record, its sweep (now
`atv_sweep::pair`), the part search and the world step all the port's. RAM outside the stack is equal
after each of 130 frames, the sweep's record is equal word for word on each hit, and the put-out runs
on the port's answer. The generated-input oracle for `atv_sweep::pair` itself is F51's (10,000 cases,
`tests/mxvatv_bike_ride.rs`).

## Not verified / not 1:1

- **The hit ride is not a played ride.** The quad's velocity is doubled by poking RAM before frame 87,
  on both machines alike. No captured C28 ride and no ride driven from the pad alone makes the sweep hit
  on Desert Oasis. The palm ride's part rays always push the quad off before the sweep point reaches the
  trunk. The frames after the poke are still the original's, word for word.
- **Only mesh hits are proved on a ride.** The sweep pairs only props, and the probe skips placed groups.
  So on a ride the hierarchy branch (the kept part `+0x84`, owner words) and the case where several
  meshes take the segment are covered only by F51's generated cases, not by a ride. On a mesh the ATV
  pair and MX Unleashed's pair give the same words. The `+0xac` word is 0 either way, and a mesh writes no
  part. So this ride alone would not tell the two pairs apart.
- **I did not count how many frames hit.** The test needs at least one hit. The scan saw the hit bit on
  frame 90 on the original machine. The test's assert counts the port's hits but does not pin how many
  there are.
- **The sweep over the whole world (`nearby` false, `atv_sweep::search_world`) is not exercised.** The
  quad's record always has a nearby list. As before, the port's outside asserts the sweep is a nearby
  one.
- **The record's other per-segment entries (1..15) are not compared.** The record sweeps one segment.
  Entries past the first keep whatever stale words the original left in them, and the port's fresh probe
  does not model those.

## Next

Nothing.
