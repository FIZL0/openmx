# S421 — rear dust follows the swingarm

2026-11-18. Task: `./kit plan S421`.

The rear tyre's dust emitters (the generic dust and the three rear sprays) now take their point from
the original's node chain: root × Bike × SwingArm × RearTire_Particle01, with SwingArm aimed at the
rear axle by the frame before's rear compression (`game_sim::dust::RearNode`). In C33's 60-tick Ee run
every rear emitter's `+0x40` matches the port bit for bit, on every tick, for all four riders, fed what
the original holds as it emits. The Sim's race4 replay still draws exactly C33's table. The one input
the Sim cannot make is Bike's own local, the chassis pose's, which moves by a few ulps; the Sim holds
the image's (S447). The front and exhaust emitters are still fixed places (S446). Details in the
numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
