# S228 — rider lean (P227, folded into S228)

2026-11-02. Task: `./kit plan S228`.

The user's complaint is that the drawn rider leans or reacts too little. The sim's half is held: on
fly_and_collide, the rider's lean and fore-aft request axes are bit-exact against the original's
p1_req_axes on every riding tick of the nine events (entry 1).

The drawn half (S228a, entry 2) is held to stated tolerances against the original's posed joints and
its riding ragdoll's parts. The clip pose matches on 937 of 1153 leaning ticks. Where the drawn
ragdoll parts, it leans less: in hard turns it reaches 0.80 of the original's lean, and the drawn
bike's root rolls 0.90 of the original's. The cause is S253; the clip partings are V121.
