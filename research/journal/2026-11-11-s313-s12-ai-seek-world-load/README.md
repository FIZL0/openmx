# S313 — s12 ai seek world load

2026-11-11. Task: `./kit plan S313`.

This task reads a free-ride map's AI targets into the seeker's `World` at load: `ai::seek::load`
takes the map's hit.dat, free.hit and free.set off the disc. On free09 it matches recording2's image
field for field and bit for bit: 106 jumps and targets, 12 start-zone groups and 4 rider sets. The
start zones' open flags and the riders are inputs (follow-ups S314 and S315). The work is committed. It
is unticked only because the gate is red on base in mxvatv_car_frame (MD132), which main has fixed.
