# S53 — the ragdoll in the ride

2026-10-20. Task: `./kit plan S53`.

**Part 1** (1-s53-the-ragdoll-in-the-ride-PART.md) was blocked on S51 and S52. It ported the wreck
sound's random draw (`wreck_sound.rs`), proved bit-exact by two 10 000-case oracles.

**Part 2** (2-s53-the-ragdoll-in-the-ride-PART.md), after S51/S52 landed, runs the ragdoll from
`Rider` while the body is down (`ragdoll_ride.rs`):
- the riding entry when the kit is built, the wreck entry and then the wreck sound on one generator
  at the wreck, and the damping switch from the mode machine;
- `hit_of` now matches the entry's per-reason switch;
- the shell draws the rider from the bones and was checked by eye: the rider tumbles off the bike
  and comes to rest.

The kit's pins equal the live actor's bit for bit (lower torso within 5e-7). recording2's reason-5
wreck settles on port tick 16 against the original's 18; the test holds 16..=20. Headless runs
carry no kit and hash as before. Part 2's Not verified list says what is not 1:1, chiefly the fixed
riding pose and the main bone, which the human's windows cannot be compared on.
