# F31b5l — frame post fade search kind clear

2026-11-09. Task: `./kit plan F31b5l`.

The tail's per-frame message post, the wreck fade's world search and the kind clear used to run whole
in the interpreter. Now all three are Rust's in C15's every-port replays:
- the post is `reset::post` on RAM;
- the search is the new `atv_search::collide_world`, which is MX's `probe_collide_world` plus ATV's
  space test and retired pass-over, with the part's box from `atv_part::bounds`;
- the kind clear is the new `update::kind_clear`/`show`.

Ride-2 and flip-1 still replay bit-exact with the RAM the handing run's. The part box and the kind
clear have function oracles. The post's eleven listeners (the HUD, race progress and others) still run
the original's code. They are shown not to touch the bike on C15, but they are not ported. The show's
switch-on and spatial insert are also still handed on. Both are filed as follow-ups. Details in
`1-frame-post-fade-search-kind-clear-*.md`.

## 2026-11-11 — the tick
The work above merged on 11-10 as a pre-gate commit (66c902b70) without the task's tick. This
session re-ran the gate: it was red on main on `mxvatv_car_frame::all_eight_pieces_are_mx_unleasheds`
(deterministic: "preload: fired, degenerate seen only 19 times" against the floor of 20), the same
rare-branch floor B36 fixed for the six-piece runs. The eight-piece run now goes 6,000 frames too.
No change to the F31b5l port itself; its Not verified list stands as written in the FINAL entry.
