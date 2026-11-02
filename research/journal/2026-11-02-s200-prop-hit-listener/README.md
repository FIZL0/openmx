# S200 — prop hit listener

2026-11-02. Task: `./kit plan S200`.

This task ported the props' ray-probe hit listener and the asleep probe's category list. The list is now
state on each prop's collision object. The disable sets it to mode 0 with the wake mask, and the enable
and the put-back reset it. A ray hit on a wake-category (0x40) object wakes an asleep prop, and every hit
activates its collection. This replaces S198's `Waking` filter. Two EE-interpreter tests in
`tests/prop_wake.rs` prove it against first_recording: one checks every probe's list, and one runs an
asleep egg on a relabelled 0x40 object for 600 frames, matching bit for bit. Three mutations fail them.
The props' world still lacks the ride's bikes and the trailers' meshes, and the sweep probe's list is not
ported; both are in S239.
