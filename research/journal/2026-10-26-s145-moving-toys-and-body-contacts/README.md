# S145 — moving toys and body contacts

2026-10-26. Task: `./kit plan S145`.

Moving props are now movers for the body contacts, not only for the wheels. The plane, the wreck
bike and the riding bike's body rays meet a moving prop at its motion. S137's car search does not
(naming the car there turns the train's nose from wreck 6 into 1, and train.rs pins 6; S170). The
wreck bike dropped on free06's train rides it (1.2 ft of slip, against 212 ft without the cars'
motion). Animated toys' collision parts carry their joints' motion. The wheel's owner branch is
ported and proved bit for bit over 20,000 cases, but not yet wired (S168). One discrepancy is open:
free09's windmill part, whose original velocity looks bogus (S167). Another is the car search's mover (S170). HasPhysics props are still not
simulated (S169). Details in 1-moving-toys-and-body-contacts.
