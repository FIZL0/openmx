# MD63d — MX vs. ATV's GravityScale and its sixteen FrictionZones

2026-11-02. Task: `./kit plan MD63d`.

Read SLUS-21104's `[Mass Properties]` reader (0x00493210). It is the rigid-body part reader that MX
vs. ATV's vehicles share, MX Unleashed's `physics_part_read` with one key per surface. `GravityScale`
is kept, and the gravity force is `-GravityScale × Weight`. MX Unleashed's bike instead has
`-(Weight/32.2) × Gravity × 32.2`, which rounds differently. `FrictionZone1..16` are the collision
record's sixteen surface frictions. `Friction` and `BodyFriction` default to 1 and to `Friction`.
The port's two stand-ins (GravityScale read as Gravity, FrictionZone1 as TerrainFriction) are
replaced by `handling::MassPart`, which `mxvatv_phy` lays over the bike. An oracle in
`tests/mxvatv_mass.rs` runs the reader itself over 10,000 generated files and compares every word.
Details in the numbered file.
