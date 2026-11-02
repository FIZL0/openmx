# S228 — rider lean (P227, folded into S228)

2026-11-02. Task: `./kit plan S228`.

The user's complaint is that the drawn rider leans or reacts too little. The sim's half is held: on
fly_and_collide, the rider's lean and fore-aft request axes are bit-exact against the original's
p1_req_axes on every riding tick of the nine events. The drawn half (game-render's rider pose against
the original's drawn joints, within a tolerance) is not done; it is split to S228a.
