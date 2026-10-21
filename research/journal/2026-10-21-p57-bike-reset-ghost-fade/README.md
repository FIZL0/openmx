# P57 — bike reset ghost fade

2026-10-21. Task: `./kit plan P57`.

After a put-back the original draws the bike, its parts and the rider see-through and fades them back
to solid. S57 could not find the timer and guessed 0.7–1 s by eye. It is a 3 s clock on the rider
object, started by every put-back of the bike (the wreck's own and L1+R1's) and stepped at the head of
the bike's update. The alpha is 0.5 on the put-back's tick, drops to 0.11 on the next, and climbs
linearly to 1. It was read live on copy 3 and ported bit for bit as `vehicle::Ghost`, which the
shell's `Garage` ticks and starts with the bike. The GPU renderer draws an `Actor` below alpha 1 with
a blend-constant pass after the view's solid and translucent draws. The M key and P41's menu switch
get no ghost, because the original's swap puts nothing back; that question is in `plan/HUMAN.md`.
Details are in 1-bike-reset-ghost-fade-FINAL.md.
