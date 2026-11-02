# S217 — queued trick pair

2026-11-02. Task: `./kit plan S217`.

Filmed a queued trick pair in the original (a generated free09 tape: flatliner off the lip, the Taunt
queued 14 ticks later) and compared it with the port's tick by tick, both skeletons. The trick timing
is 1:1 (the switch on the same tick, at the same elapsed), and so is the rider's pose across the switch
(within 1.5°). Nothing snaps on either side, so S214's fix holds. The bike is 1:1 except for the 24
ticks after the switch: the original's bike motion object (found at the rider's +0x68) restarts its
clip and fades from a snapshot over 10 ticks, while the port's bike crosses over the body's 24 ticks
(up to 50° on the root; filed as B10). The comparison is `trick_chain.rs`
`a_queued_pair_matches_the_original_across_the_switch`. Details in `1-queued-trick-pair`.
