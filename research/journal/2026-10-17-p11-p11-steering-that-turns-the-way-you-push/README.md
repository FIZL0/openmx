# P11 — p11 steering that turns the way you push

2026-10-17. Task: `./kit plan P11`.

The bike steered the wrong way and would not come back up, and neither half was a sign in the
lean-to-steer arithmetic. The steering command was never wired to the front wheel at all — the pose
wrote it to one struct's copy of the word and the contact patch read the other's, so the bike was
never steered and what the report watched the heading do was the terrain doing it. The lean was a
reflection: `ride.rs`' `set_axes` built the bike node's rows as `forward × up`, determinant −1, where
the original's own orthonormaliser builds `up × forward`, so `move::no_hander` read back the negation
of the roll it had just written, fought itself, and settled at a twentieth of the commanded lean —
leaning out of the turn instead of into it. Two lines, and then four tests that would catch either
again: a thirteen-second held turn asserted second by second, a stick-centred bike coming back level,
and the lean-against-heading handedness read out of the human's recording. P10's two ignored steering
tests (one of them through the pad) are paid and un-ignored.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
