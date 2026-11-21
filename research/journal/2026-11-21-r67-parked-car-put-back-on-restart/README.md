# R67 — parked car put back on restart

2026-11-21. Task: `./kit plan R67`.

Yes: the original puts every parked car back again at each world reset. The pause menu's Restart runs
one, and so does the world load, which is the first put-back. Each put-back squares the stored nose and up
again, so restart N turns the car by the pair squared N+1 times. This was run in the interpreter over
R52's free07 baja image, and it gave S498's twice-squared bits after one reset. Fliers are not put back
by it. Filed S500 (the port change) and R68 (a driven car's store moves with it). Details in
1-parked-car-put-back-on-restart-FINAL.md.
