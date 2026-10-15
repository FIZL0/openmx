# S1 — s1 fixed update

2026-10-15. Task: `./kit plan S1`.

The game's frame, ported as `crates/game-sim/src/tick.rs`: `Engine::step` services one mode request,
samples the Timer 0 clock, pushes the raw delta through a thirty-entry moving average with its 30 fps
clamp and its time scale, latches the pads, raises the top-of-frame marker, and only then simulates —
the order everything else in this crate will hang off. It is proved twice: against the original's own
`engine_tick` and `game_tick` running in the oracle, over 12,000 generated frames and 10,000 clock
samples, with four hooks and one patched `lui` to make a frame that reads a hardware register callable
at all; and against C4's 600-frame recording of the game running, replayed from its clock column alone
and matching dt, the whole ring and 1/dt bit for bit. Two earlier findings were corrected along the way:
the engine tick's return value is discarded by the caller, and the flag the end of the frame latches is
not the simulation gate but a deferred flag one byte along. The gaps are in `1-s1-fixed-update-FINAL.md`
— the ones that matter later are that every mode handler and the whole renderer are hooked, so what a
transition *does* is still unproved, and that no live frame-by-frame comparison against a running game
has been made yet.
