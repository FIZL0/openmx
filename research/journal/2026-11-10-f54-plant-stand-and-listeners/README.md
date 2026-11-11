# F54 — plant stand and listeners

2026-11-10. Task: `./kit plan F54`.

The part search's two handed-on calls, the plant stand and the riding part's two listeners, ported to
Rust (src atv_plant, atv_hear) and proven by oracles over 10,000+ inputs each and by C15's ride-2 and
flip-1 replaying in lockstep, RAM alike, with nothing handed on by name. The first session's work was
saved on wip/F54 uncommitted; the second (2026-11-11) brought it onto main past S289, merged the oracle.txt
sections, re-ran the tests (and probed that the replay and the oracles ran, not skipped). Details in 1-…-PART: committed unticked, as the gate is red on base (MD132).
