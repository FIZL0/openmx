# S191a — head hits on cars

2026-11-01. Task: `./kit plan S191a`.

S191 left two of its three train rides wrecking for a different reason than the original. The port's
head ray seemed to strike a car where the original's did not. There were two causes, and neither was the
ray test. On stand, the head entry was the port's own: `mover_contact`'s head-inside-a-car stand-in
(S137, S216). The original has no such entry, so the stand-in is gone. On roof, the original's head ray
strikes the car as well, but the nose-dive's wreck entry clears the collision record before the hit
loop reads it. The port had let the hit loop's reason override the nose-dive instead. With both fixed,
all three rides agree with the original to their wreck ticks and take the original's wreck reasons
(headon 1 on 60, stand 6 on 93, roof 8 on 158). Details in the numbered files; the newest one's suffix
says whose move it is (FINAL · PART · READY).
