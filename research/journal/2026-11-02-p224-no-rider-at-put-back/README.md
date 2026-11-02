# P224 — no rider drawn at a put-back

2026-11-02. Task: `./kit plan P224`.

The only phone log from a build with P212a's line (10-30, build 2366) never showed a bike without a
rider for a whole launch. Its three `(no rider drawn in N of M frames)` lines were the put-back tick on
which the original still draws the body where the ragdoll lay (S74). That frame is no longer counted. The
real 10-28 bug is still waiting on a log from a launch that shows it. Details in the numbered files; the
newest one's suffix says whose move it is (FINAL · PART · READY).
