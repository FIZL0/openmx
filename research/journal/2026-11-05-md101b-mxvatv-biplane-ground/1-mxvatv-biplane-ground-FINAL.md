# 1 — mxvatv biplane ground (FINAL)

Task MD101b. 2026-11-05.

## What I did

- `handling::ground::Under`: atv_ground's `Terrain` as a `world::World`, so a `Flight::new_mxvatv` can
  stand its tires on MC02's ground. `ground()` is the query, `surface()` the query's record byte, and
  `solid()` is false.
- `biplane::Measure::flight_mxvatv`: SLUS-21104 builds the plane matrix (plane+0x370) the tires read with
  its own quaternion-to-frame routine (`atv_moto::quat_frame`), not MXU's. The two differ by 1 ulp, which
  had made every tire offset 1 ulp off, even in the air. `Flight::remeasure` calls it when `mxvatv` is set.
- `biplane::Tire::strut_mxvatv` / `force_mxvatv`, plus `WheelBody::add_velocity_quiet`: past full
  compression, SLUS's strut writes the body's velocity directly. It does not go through the body's
  add-velocity entry, so the shock words (+0xc0..) stay 0 on a landing. That entry is what made the
  landings part (biplane 1259: body+0xc4 10.92 in the port, 0 in the game). The default
  `add_velocity_quiet` is `add_velocity`, so `tests/biplane.rs`'s `Logged` body is untouched.
  `strut` and `strut_mxvatv` share `strut_with(.., quiet)`.
- `tests/mxvatv_biplane_replay.rs`:
  - the seed reads all three tires off `*(plane+0x116c+4k)` (`read_tire`; the strut's .phy words sit at
    +0x30..+0x40, 0xc lower than MXU's);
  - the port flies over `Under(terrain(MC02))`, and the test checks that the three tires share one
    terrain object whose words are `Terrain::of`'s;
  - every frame compares the three tires word for word with the game's (`tires_diff`: the quads, words
    and flags at the offsets in `TIRE_QUADS` / `TIRE_WORDS` / `TIRE_FLAGS`, and the record byte +0xb0);
  - ratchet raised.
- research/oracle.txt `# MD101b`. Follow-up MD110: the hull on the terrain.

## What the original does

- **The tire strut:** `./kit fn -p atv 0x0049d720`, a0 = dt, a1 = the tire.
  - The hub and the bottom are taken through plane+0x370.
  - The compression runs along the hub−bottom line to the ground.
  - The fade is clamp(((y − 0.5) / 0.13999999)·(1 − floor) + floor + 0.0), where y is the turned normal's
    y with its z dropped and the result normalised. This is the port's `remap`, bit for bit, on every row.
  - Past +0x30:
    - the compression is clamped;
    - the position moves by n·(0·0.9);
    - body+0x80 += n·(−min(v·n, 0)), written directly;
    - body vt+0x80 is called with n·(−min(f·n, 0)).
  - vt+0x80 is 0x00493ff0, a thunk to *(body+0x110)+0x48 = 0x00494000 (`./kit dis -p atv 0x00493ff0`).
    That routine does force (+0xa0) += f, then calls the body's listeners (+0xe8 of them), i.e. the
    port's `Body::add_force`.
  - Then come the spring and damper through vt+0x84 and the decompression limit (+0x34·dt). The last
    compression is min(c, +0x30).

## How it was proved

Bit-exact replay of MD101's two captures (`context/mxvatv-ps2/md80a/{biplane,biplane3}.ee.bin` with
their tapes) in `tests/mxvatv_biplane_replay.rs` `the_port_flies_biplane` / `the_port_flies_biplane3`.
A port Flight is seeded every 100 frames and flown beside the interpreter, with the body and all three
tires compared word for word after every frame.

- **biplane:** all 17 stretches agree to their end (16 × 100 frames, then 30 to the capture's end). This
  covers the take-off (stretches from 0 and 100, grounded) and the landing (frame 1259) and roll-out
  (1300–1630).
- **biplane3:** 22 of 24 stretches agree to their end. These cover the take-off and the roll-out from
  2200 to 2339.

Before MD101b the pins were [0,0,100×10,58,0,0,0,0] and [0,100,…,59,0,0,0].

Mutations, observed while finding the fixes (not re-run as a final check): with the shock-taking `add_velocity` (the MXU strut), biplane's
landing stretch drops from 100 to 59 frames. With MXU's matrix, every tire offset is 1 ulp off.

## Not verified / not 1:1

- **biplane3's frame 2059 and its stretch seeded at 2100 (59 and 0 frames).** The game's body takes a
  force and a shock no tire gives, and the tires agree on that frame. I inferred that this is the
  plane's hull colliding with the terrain, from the shock words (+0xc0) and the force (+0x90) being
  nonzero while the grounded word is 0. I did not trace the writer. `Under::solid` is false; follow-up
  MD110.
- **`Under::ground`'s normal buffer starts straight up,** not at the tire's previous +0xd0 as the game's
  probe hands it. The words agree on every compared row, but no row was found where the query leaves
  the buffer unwritten, so that path is unproved.
- **`Under::surface` answers 0 where the query writes no record.** It matches on every row, but only
  hit rows were seen.
- **The fade floor's 1.0-clamp branches** (floor ≥ 1) never run with biplane.phy's floor, so they are
  unproved.
- **The landing's friction** (`Tire::roll`, SLUS 0x0049dd40) is compared only through its outputs on these
  rows. Its brake/steer branches at full deflection are covered only as far as the tapes push them.

## Next

The hull on the terrain (MD110), then the wreck machine (MD101c).
