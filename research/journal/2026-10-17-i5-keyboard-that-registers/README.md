# I5 — keyboard that registers

2026-10-17. Task: `./kit plan I5`.

The shell's keyboard was unusable in two ways at once, and both were the host's input taken too
literally. `window.rs` kept key state straight off the X11 wire, where a held key is not held: core
auto-repeat delivers a release and an immediate re-press thirty times a second, so `held()` blinked
false and the simulation's edges saw taps instead of a hold. The key state is now a `Keyboard` that
believes a release only once it has seen what came after it — a repeat's two halves cancel, and
nothing outside the struct changes (`ChangeKeyboardControl` would have turned repeat off for the
user's whole session, so it was not used). `pad.rs`'s keyboard sticks answered only 0, centre or 255,
which made lean, steer and preload all-or-nothing through I2's dead zone; each of the four axes is now
a `KeyAxis` that ramps to its edge in 0.18 s and comes home in 0.10 s, timed in seconds so the feel
does not follow the frame rate. 471 tests pass, 6 of them new: an event stream with repeat pairs in it
leaves the key held, and a held direction crosses the dead zone on frame three and reaches the stop
around frame eleven. What is not proved — chiefly that nobody could press a key at this display — is
in the entry's last section.
