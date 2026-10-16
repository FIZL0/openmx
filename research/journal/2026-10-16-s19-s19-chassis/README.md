# S19 — s19 chassis

2026-10-16. Task: `./kit plan S19`.

The rigid body under the bike, ported and proved: the integrate virtual the ground step ends every
substep with, the per-frame speed measure, the transform's rotate/unrotate pair, the two-step
integrator the body calls through a pointer on itself, the three freedom flags the step sets, and the
contact distributor that spreads a frame's collisions onto the two wheels. It came out clean — all
0x330 bytes of the body compared word for word against the original over 10,000 generated cases for
each of the two whole-object steps, with the ordered list of what each one called compared too, and
every branch counted. Two findings worth carrying: the unrotate's transpose is finished by a plain
64-bit `move` that keeps the upper half of its destination (read as a quadword copy it looks like the
original has a bug, and it does not), and the step's correction block is gated by a branchless
quadword compare that needed two more MMI ops in the interpreter. What is *not* proved is the
transform's own arithmetic — six callees are hooked and `integrate` returns the call list instead,
the same seam S3 left behind.
