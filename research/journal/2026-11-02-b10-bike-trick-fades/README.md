# B10 — bike trick fades

2026-11-02. Task: `./kit plan B10`.

The port's bike now fades into a trick the way S217's capture shows the original's bike object
doing it. A trick that follows another trick restarts its own clip and fades from a snapshot, where
before it took the body's 24-tick stunt cross. Every trick fade runs at a rate of 3 (10 ticks, not
the body's 7.5). Each tick is drawn at the weight the tick before left, so the restart tick shows
all of the snapshot. `trick_chain.rs` `a_queued_pair_matches_the_original_across_the_switch` now
holds the bike within 2° from tick 10 to the landing, across the switch. The worst tick is 1.30°,
against 50° on the root before. Details in `1-bike-trick-fades`.
