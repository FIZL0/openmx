# B27 — one voice per knock

2026-11-03. Task: `./kit plan B27`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

F27a1 found the free09 ball knock queued on two consecutive ticks in the original but heard as one
bbhit voice, while the port's `HitTable::sound` stacked one voice a tick. The brief asked for the
rule in the sound object's play that drops the repeat. There is no such rule: the sound class's
start always allocates a new voice and never stops the old one. What keeps the knock to one voice is
the "speed" material_hit_play reads. It is the owner's impulse, not its velocity: the body's shock
peak, or with no body the chassis' impact accumulator. Both are the frame's own and are cleared each
frame, so only the strike frame reaches MinImpulse. RAM images 2903–2906 of f27a1 prove it (16.81 →
level 67 on 2903, then 0, and one voice handle that only ages). The port now reads the same:
`Body::shock_peak` for props and planes, and a new sound-only `Ride::impact` (the frame's
`mvc.accel`) for the bike. The knock no longer stacks, but in the port's ride it is now silent,
because the port hears the ball's own probe entry a tick after its shock peak. That is B28. Done;
details in `1-knock-one-voice-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/src/material_hit.rs` | `Impact`, rule 1's docs: the speed is the impulse |
| `crates/game-sim/src/ride.rs` | `Ride::impact`, set after the frame |
| `crates/openmx/src/main.rs` | `HeardRide::owner` reads impulses |
| `crates/game-sim/tests/material_hit.rs` | `a_knock_sounds_once` on the four RAM images |
