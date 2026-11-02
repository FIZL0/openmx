# C15 — mxvatv ride capture

2026-11-02. Task: `./kit plan C15`.

Recorded two MX vs. ATV (SLUS-21104) Free Ride bike rides from save state 4 (copy 9's), for the future
bit-exact replay of Handling::MxVsAtv. uinput is closed again, so each ride is a generated input tape
(research/c15/tape.py) played in PCSX2 copy 1 and captured every tick with research/c15/bike.toml. `ride`
is clean (gas, two jumps landed, steer both ways, brake, gas, coast; no wreck). `flip` fires the R1
backflip off the first lip, over-rotates, wrecks and is put back. Each tape was played twice, and the two
runs are byte-identical in every column, raw blocks included. The captures and tapes are git-ignored in
s1's context/. Details in 1-mxvatv-ride-capture-FINAL.md.
