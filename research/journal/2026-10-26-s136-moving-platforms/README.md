# S136 — moving platforms

2026-10-26. Task: `./kit plan S136`.

A bike on the free06 train's roof now rides with the train. Each path car keeps its motion over the
tick (`traffic::Moving`): its velocity and spin, made from its poses before and after the step, as
the original's car virtuals make them. A wheel standing on the car subtracts that motion from its
contact velocities, as the original's wheel_contact_velocity does. Both are proved bit for bit
against the original's functions on s89-along: 6300 car-ticks and 20,000 generated wheel cases.
With air drag off, a parked bike stays within 0.08 ft on the deck for five seconds. With the stock
bike's drag, which works against the world, it slips back at 8 ft/s and leaves the deck after three
seconds instead of one. No capture of the original on the train exists yet (S129), so that slip is
unconfirmed. Moving toys and body contacts are not done. Details in 1-moving-platforms.
