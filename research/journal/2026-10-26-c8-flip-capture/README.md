# C8 — flip capture

2026-10-26. Task: `./kit plan C8`.

C8 made a free09 tape for V35: an uncut backflip and a no-hander, captured on the original. The tape
is recording2 patched (`capture/make_tape.py`): flip 1's Taunt is lifted with the stick centred under
it, so the flip's torque holds as it did, and the cancan on the next jump becomes Triangle + up, the
no-hander. Two runs of `capture/play.sh` with `capture/c8.toml` agree: backflip clip (state 38)
2512..2579 uncut, no-hander (trick 4) 2651..2696, both landed without a wreck; the film's HUD shows
"Backflip" and "No Hander". The no-hander is let go in the air, so the landed no-hander (state 39) is
not in it. V77 puts the capture into V35's test.
