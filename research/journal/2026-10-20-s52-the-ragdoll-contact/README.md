# S52 — the ragdoll contact

2026-10-20. Task: `./kit plan S52`.

S52 ports what keeps the thrown rider's bones out of the world. That is the nine bone collisions
(ground, scene and sweep, with the push-out and impulse) and the bike box. It also ports the two
virtuals both ragdoll entries begin with: the pose, which puts every bone body where the skeleton's
bone is, with a velocity and rates over the frame, and the reset. Each is proved bit-exact on the live
rider body actor, over 10 000+ generated cases.

The settle test is now a clearance against RiderFlailHeight, as the original has it, and its three
sensors can be read off the bones (`Thrown::from_bones`, proved in the wreck-arm frame). The ride
itself still feeds them from `Crash`'s one point until S53 runs the ragdoll in `Rider`.

Details are in 1-the-ragdoll-contact-FINAL.md.
