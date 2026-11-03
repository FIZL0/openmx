# C17 — the Kiowa's rotor strike

2026-11-03. Task: `./kit plan C17`.

**The port.** The crash test's strike now asks the rotor's own collision object
(`Collision::rotor_struck` in `crates/game-sim/src/kiowa.rs`). The rotor's eight wreck rays are built
into a ray-probe tree as the original builds one (`ray_tree`). That tree is searched at the measure's
matrix against the helicopter's nearby list, as a ray object of category 0x80000 that never collides
with 0x100 or 0x100000. The helicopter wrecks when the first contact's object has a category outside
the struck mask 0x40000.

**The proofs.**
- The tree build against the original's code on 1,999 random ray lists (`tests/kiowa.rs`).
- The rotor's tree against the one in C14's image (`tests/kiowa_capture.rs`).
- A captured strike that replays bit-exact up to and including the wreck tick (`tests/kiowa_capture.rs`).

**The capture.** A new tape from C14's state: climb a little, settle about 7 ft up, then left rudder.
The original wrecks on frame 1298. The port, with nothing tuned, wrecks on the same tick, by the rotor
search alone: the struck object is tree 138's trunk. The capture does not record what the original
struck (`1-rotor-strike-FINAL.md`).
