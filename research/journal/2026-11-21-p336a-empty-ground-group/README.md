# P336a — empty ground group

2026-11-21. Task: `./kit plan P336a`.

Rode sx01 in Safari on the iPhone (WebGPU, beta 998d107) and read the page log off the phone: the stage
"the views" broke with "number of bind groups set(2) is less than the pipeline uses(3)". The ground's pipeline
layout left group 2 missing when a map has no roads and ruts; Safari wants a bind group there, Chrome does not.
Group 2 is now an empty group, set in the draw. Proved by a test on this machine; the ride on the phone waits
for the deploy. Details in 1-empty-ground-group-FINAL.md.
