# MD233 — untamed tire on object

2026-11-17. Task: `./kit plan MD233`.

A truck tire standing on a loose prop in MD160's Untamed throttle drive. Untamed's tire step reads what a
tire stands on exactly as SLUS-21104's does, so the port's tire step needed no change: the Untamed harness
now answers the port's ask from the original's RAM and checks the centre of mass it writes back. The
throttle drive's five skipped frames (161, 258-260, 268) are compared bit-exact, all 310 frames. Details in
1-untamed-tire-on-object-FINAL.md.
