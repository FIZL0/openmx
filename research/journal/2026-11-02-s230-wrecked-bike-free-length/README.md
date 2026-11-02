# S230 — a wrecked bike springs back to its unloaded stance

2026-11-02. Task: `./kit plan S230`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

After a wreck, and most of all after falling off the back of a wheelie, the riderless bike stayed squatted
on its suspension. The cause: the ground step does not run while the rider is down, so both wheels kept the
compression of the wreck's tick, and the shell draws the fork and swingarm from that number
(`game_render::vehicle::Drive::of`). The original's vehicle wreck entry (`bike_wreck_entry`) zeroes both
wheels' compression (`wheel+0xf0`) once it has handed the body over. The port's entry comment listed this
as "the wheel loads zeroed … not ported". It is ported now (`Ride::wreck_wheels`, called from the rider's
wreck entry), so a downed bike is drawn at its free length. Done; details in `1-wreck-wheels-FINAL.md`.
