# S220a1a — kiowa blades

2026-11-02. Task: `./kit plan S220a1a`.

This task gave the Kiowa's four thrown blades bodies and a world to fly in. They are
`game_sim::kiowa::{Blades, Started}`: each blade is a props `Collection` object built off
`wreck_blade`'s part, `bladebent.psg`'s box and `bladebentm.col`'s rays. The task also ported
wreck_part_start and wreck_part_stop's insides, and `Wreckage::throw_blades` now places and starts
them. tests/kiowa.rs checks it against the original on S148's state:
- the build, word for word;
- the start, stop, throw, wreck and put-back over 10,000 cases;
- 40 throws flown for 60 frames each beside the original's world step and object listeners.

All of it is bit-exact. A fix to `prop_world::Collection::disable` came with it: the world's lists
drop an entry by swapping in the last one, not by shifting the rest down.

Nothing draws the blades yet, and openmx does not call any of it, so there is no on-screen look.
Details in 1-kiowa-blades-FINAL.md.
