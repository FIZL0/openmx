# A5 — a5 whole frame

2026-10-16. Task: `./kit plan A5`.

S6 ported the rider's animation driving one function at a time and ended by naming what that cannot
catch: an ordering mistake between the animation header and the rest of the rider update. S8's answer
— replaying a human's recording tick by tick — is blocked on a person at PCSX2's GUI. So this task
built the whole-frame proof that is reachable without one: `watch/frame-bike.toml` samples the human's
rider once per tick on the live emulator, from the save state the recording is anchored to, with the
kit's own pad riding it (`watch/pads/frame-bike-ride.pad`), and each row carries the physics' outputs,
the attachment walk's seat position beside the chassis' transform, and the six fields the animation
header writes. Over 1,400 consecutive ticks — 513 of them airborne, 22 cross-fade restarts, two
wrecks — the port's `motion::Driver` reproduces all six fields **bit for bit on every tick**, and the
same replay handed the previous tick's physics breaks on 1,092 of them: the header reads the physics of
its own frame, measured rather than argued. Two findings came out of the capture that the per-function
proofs could not reach: `MaxCrossupAngle` is 45 degrees (`0x3f490fd9`), which closes an item in S6's
"not verified", and the rider's reset puts the shown crossup back to 0.5 — which `Driver` does not do,
so a port would carry a stale crossup through a wreck and the landing test reads that field. The
chassis itself is still not compared numerically (the port has no bike), and the attachment walk's
order is proved by a distance rather than bit-exactly; both, and the rest, are in
`1-a5-whole-frame-FINAL.md` under "Not verified / not 1:1".
