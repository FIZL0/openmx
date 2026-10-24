# S67 — landing snap 2

2026-10-24. Task: `./kit plan S67`.

Landings still felt snappy after S55. Animandpump's landings were ridden on the original and on the
port from the same save states and the same pad, tick by tick. The snap was the bottom-out shove: the
original moves the body out of the ground through the transform, and the port had dropped it, so the
port's springs threw the bike back up by a foot or more. A second gap was that the frame did not end
with the chassis quaternion re-read off its node, as the stunt clocks' copy-back does. With both
fixed, six of the seven captured landings ride within 0.33° and 0.023 of the original, and so do the
drawn pose and the camera's axes. The nose-first drop still parts at its touchdown tick (a
follow-up). Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
