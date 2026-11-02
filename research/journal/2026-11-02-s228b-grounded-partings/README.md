# S228b — fly_and_collide's grounded partings

2026-11-02. Task: `./kit plan S228b`.

On the fly_and_collide tape, P1 parted from the capture on the ground at speed with the stick
moving, in nine 100-frame stretches. Eight of them, and a ninth blamed on a preload, came from one
cause: the ground step's donut read an angle the port worked out for itself, not the pad read's.
The fix is in `ride::frame_searched` (entry 1). Two partings remain, both where the capture shows a
chassis force on the ground that the port does not make. The put-backs were not looked at.
