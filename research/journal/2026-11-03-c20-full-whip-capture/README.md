# C20: the original's riding body through a full whip

The original's riding ragdoll was captured through a full whip on free09 jump 2: the stick hard
right from just after the take-off to the landing, with no trick held. The port's riding body was
then compared against it.

- [1-full-whip-capture-FINAL.md](1-full-whip-capture-FINAL.md): the capture, the comparison, and
  which way the original's body goes (it rises).
- `capture/make_tape.py`: builds the tape from recording2. `capture/play_whip.sh`: plays it under
  S201's `entry_watch.py`.

Test: `crates/game-sim/tests/riding_spring.rs` `a_full_whip_hangs_the_riding_body_as_the_originals`.
