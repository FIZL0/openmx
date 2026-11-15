# S321 — flashes on the race generator

2026-11-15. Task: `./kit plan S321`.

A supercross stadium's camera flashes (V127's `camera_flash::CameraFlashes`) now tick inside the
simulation: the object is the race's (`race::Race::flashes`), ticked at the end of `Race::tick` on the
race's generator, which is where the original's flash object ticks (a child the score manager's tick walks
last). The shell hands its object to the first seat's race when it builds it, and draws what the race's
object shows. Held against S13d's capture's `rand` state: 454 batches, none drawing more than the capture's
generator moved, and 114 of them (the frame's only draws) landing on the row's state bit for bit.
