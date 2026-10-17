# P5 — replay seam

2026-10-17. Task: `./kit plan P5`.

The harness drove A1's example ball while a person rode `ride::Ride` behind the shell's own world, so
`replay::verify` and the game were two different simulations and S8's acceptance verdict was
`NoSuchField 'bike.pos[0]'` at tick 0. `Sim` — the one `impl Game` — is now the ride, the rider and the
pad the shell builds, the world is an argument rather than a field so the shell can keep its own, and
the state walk names the capture's eight bike lanes. An acceptance capture is a real comparison from
here: a synthetic one verifies, and one moved bit fails as a tick and a field. The shell was rewired
onto the same value and its tape still replays bit-exact (`ride hash 0x3140102cddb3e085` recorded and
played). Details, and the five things this left unproven — chief among them that no acceptance capture
exists in this worktree, and that `Sim::new` has to turn the wreck test off to keep a spawned bike from
wrecking on its first frame — in `1-replay-seam-FINAL.md`.
