# C45 — parked bike in the node world

2026-11-20. Task: `./kit plan C45`.

Whether R63's knocked parked bike joins the level node's dynamics world because it was knocked. It
does not: it is a physics object of that world from being parked, awake and resolved beside the car
on every pass for sixty ticks before the knock (interpreter replay bit for bit against R63's capture).
R22's long-parked bike is asleep and out of the world's bodies. The port takes a parked bike out of
the world (P102), so it differs; S495 (`later`) is filed for a person to decide.
