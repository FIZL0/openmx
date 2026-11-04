# S188 — free11 path cars

2026-11-03. Task: `./kit plan S188`.

S128 proved the path cars' ground switches only on free09 and free09_ss. This task runs S128's three
interpreter comparisons on S196's free11 RAM image as well. free11 has twelve cars: two presample,
the other ten ask the ground every frame, and four of those ten are `HasPhysics` cars. All twelve match
the original bit for bit over 600 ticks. Only the test changed; the port needed no fix. Details in
`1-free11-path-cars-FINAL.md`.
