# 1 — s12 ai avoidance (FINAL)

Task S12c. 2026-11-06.

## What I did

- Ported the motocross AI's avoidance pass as `game_sim::ai::avoid` (crates/game-sim/src/ai.rs), with
  `AvoidTuning` (the constructor defaults), `Me`/`Other`/`Steer`, the `flag` bits, the `look` gate and
  `feeler_fan`. Bit-exact through F/Vf: the VU dots go mula/madda/madd, the normalise is the VU
  rsqrt × q, the fan's table lookup keeps the original's own multiply order (not `CurveTable::at`'s).
- Oracle `avoidance_matches_over_ten_thousand_passes` in crates/game-sim/tests/ai.rs runs the original
  pass in Ee 10,000 times against the port.
- research/oracle.txt: the S12c note under `ai_update_mx`, and the names `ai_mx_construct`,
  `ai_vehicle_construct`, `ai_character_read`, `path_locate`, `path_segment_holds`, `path_node_direction`.
- Follow-ups: the AIChrctr.aip avoidance keys and the path queries behind KEEP_ON_TRACK were filed as
  S281/S282 on the first branch; those ids went to S12b's splits on main, so on the merge (file 2) they
  became notes on S283 (the character reader) and S282 (the route follower). The generic VehicleAI pass is S301.
- opentennis: nothing to take (its ai.rs is tennis shot choice). The PC exe: not read; the PS2
  disassembly was enough.

## What the original does

`./kit dis 0x00386d30 0x003880d0` (ai_update_mx). Arguments: a0 ai, a1/a2 ignore count and list,
a3 position, t0 heading, t1 the steer direction (in/out), t2 the speed, t3 the aimed-for speed (out).

- Takes the MX path when ai+0xd0 == 2, ai+0xd4 (own bike) is set and own+0x40 is non-zero. Otherwise it
  calls ai_update_vehicle (`./kit fn 0x002858d0`), which is not ported. Returns if ai+0x40 (the scene)
  is null or the speed is `<= 0`. A NaN speed goes on.
- **The feeler fan and the scene walk are dead for the outputs.** When (flags&0x80)==0 || (flags&0x40)==0,
  it builds the fan into the feeler bundle (ai+0x2a0): n = +0x2a4 feelers, angle
  π/4·(0.5 + (i − n/2))/n, origin y + 3.0, length 100. Then it walks the scene through vt+0x2c/+0x30/+0x34
  (begin/next/end):
  - type-2 objects that are collidable and not ignored are swept (bundle_sweep_object) when 0x80 is clear;
  - type-3 groups are taken in (group_part_count, world_manager+0xd4) when 0xc0 is all clear.
  The hits only reach sp locals that nothing reads after the walk; the heading×speed velocity it builds
  first (sp+1344) is dead too. So the gate only changes what lands in the bundle.
- The rider loop, over race (ai+0xd8) +0x34 linked by +0x1c8, runs whatever the gate says. It skips the
  own bike, inactive riders (+4) and, with flag 0x100000, riders whose +8 has bit value 4. It then splits
  the offset into along/side, and the relative velocity (rider +0x110 − ai+0x50) into closing/drift:
  - **near**, dist < 2·√NearCollisionRange: inside the front/rear window, separate sideways and match
    drift. It brakes to the rider's speed × response when the rider is beside us within the near range,
    we are ahead on progress (own+0xbc vs rider+0xbc), we are not much faster (PassingSpeed), and the
    rider is not slower than the min braking speed. This is the brake short of a standing human: a
    rider at speed 0 is under the min braking speed, so with the defaults the near brake does not fire
    for a standing rider.
  - **far**, dist < √FarCollisionRange: if ahead within the radius and closing, swerve by the side
    direction × closing × response. It brakes to MinBrakingSpeed × response when the time to reach the
    rider is within the approach limit, the held speed (+0x80) is at least the min, and the headings
    differ by more than 45° (cos < 0.707).
  - The pushes are averaged into the steer; the brakes are averaged into t3.
- Flag 0x20000: the off-the-track push via the three path queries (oracle.txt). Then steer.y = 0, VU
  normalise, flags &= ~0x100.

## How it was proved

Function oracle over 10,000 generated passes (crates/game-sim/tests/ai.rs,
`avoidance_matches_over_ten_thousand_passes`). Each pass runs the whole original `ai_update_mx` in Ee:
- **Generated:** tunables (a quarter of the passes use the defaults), 0–8 riders around the bike (the own
  bike sometimes in the list), inactive and marked riders, a scene of type-1/2/3 objects with an ignore
  list, and every flag bit.
- **Hooked and scripted:** the scene vtable, the sweep (random answers and hits), the group count, and
  the three path queries.
- **Compared:** steer direction (4 lanes), aimed-for speed, flags, every feeler's direction/origin/length
  (or the bundle untouched), the swept set, the scenery count, and the point the path test asked about.

In one run: 7092 fans, 738 brakes, 1418 edge pushes, 522 idle passes. Every pass matched.

Mutants:
- Dropping the drift term fails at case 0.
- Regrouping the fan angle's divide fails at case 22.
- Two others survived, and neither is a real difference:
  - swapping the factors of one product: a single multiply gives the same bits either way round;
  - regrouping step·(sin·frac): the term is about 1e-3, so its ulp is far below cos's.

## Not verified / not 1:1

- **The generic VehicleAI pass (ai_update_vehicle) is not ported** (S301). `avoid` is only the ai+0xd0 == 2
  path. I did not check in RAM that free09's AI bikes have +0xd0 == 2; the MX constructor is the only
  one seen to set it.
- **The fan/scene sweep's side effects are not ported:** the bundle's record, its hit arrays and the
  sweep calls. They are output-dead in this function, so the port only returns the fan. The fan's w lane
  is not compared (the original keeps the heading's w through the normalise; nothing reads it).
- **The tunables are the constructor defaults only.** AIChrctr.aip's reader is not ported (S283, noted). The
  constructor's FarCollisionRange of 0 (√0 → the far branch never fires) may well be what ships, and that
  is not known. The near response's difficulty interpolation is not ported.
- **The field names are guesses from the key names and use:** `speed` (t2) vs `held_speed` (+0x80),
  `AVOID_DUE` (0x100), `SKIP_MARKED`, `progress`.
- **The path queries are a caller closure (S282, noted)**, so KEEP_ON_TRACK does nothing in the sim yet.
- **The scene-null early return (ai+0x40 == 0)** and the non-MX dispatch are left to the caller.
- **No live free09 capture.** The brake/swerve were not watched beside the original; that is S12e's capture.
- **NaN corner cases** are kept as the disassembly orders its compares, but no NaN was generated.

## Next

S12b/S12e wire `avoid` into a ridden AI. S282, S283 and S301 are the gaps above.
