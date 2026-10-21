# S78 — rider rider collision

2026-10-21. Task: `./kit plan S78`.

Riders pass through each other in the port. The original gives each riding bike a collision record on its
chassis: eleven body rays that strike other bikes' kind-5 collidables, with the impulse shared through the
same contact resolution S42 ported. The whole task needs the bike's collidables off the disc, the record
wired into the ride, and a two-player capture, and that was too big for one session. So it was split. This
session read the record's wiring and ported the half of the resolution the plane never used, where the own
side is a vehicle chassis (`contact::resolve_ridden`). An oracle test proves that half bit for bit over
10k cases. The rest is filed as S86 (collidables), S87 (wiring) and S88 (capture), and S78 is blocked
until they land.
