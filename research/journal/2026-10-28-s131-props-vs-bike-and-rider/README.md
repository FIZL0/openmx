# S131 — props vs bike and rider

2026-10-28. Task: `./kit plan S131`.

S131 asked for the knockable props in the ride: the node box from the model, each prop's f.col as a
moving collidable with contacts both ways, the props in `Sim`'s hash and rollback snapshot, and the shell
drawing them at their body pose. Too big for one task, so it was split (AGENT.md): the node box from the
`.psg` (part 1) and the props in the ride — loaded off the disc (`PropSet::load`), ticked with the cars,
hashed, snapshotted, drawn at their nodes (part 2) — are done and proved against free09's first_recording
state; the contacts went to **S197** (f.col struck by bike, rider, wreck bike and plane, struck listener
wake, two-sided push) and **S198** (a prop's own m.col rays and sweep against the ground). Until S197
nothing in a ride wakes a prop.
