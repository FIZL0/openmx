# MD170 — untamed golfcart replay

2026-11-19. Task: `./kit plan MD170`.

MD81e's five Untamed golf-cart drives now replay bit-exact on MD161a's harness: throttle (with its hop),
throttle2, analog (the Cross pressure sweep), steer, and jump (its landing wrecks the cart; put back on
frame 296). Every row is SLUS-21701's frame in the interpreter, but the recorder's own misreads on
throttle2 and jump. Every frame of the port's Untamed frame and Ride equals the original's word for word,
on the original's ground and on the port's terrain. The harness learned rows the recorder missed (stepped
by vsync) and pressure presses on the tape. atv_car.rs did not change. Details in
1-untamed-golfcart-replay-FINAL.md.
