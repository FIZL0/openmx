# S228c — fly_and_collide's wrecks through the original's whole frame

2026-11-03. Task: `./kit plan S228c`.

S176's whole-frame judge stalled on fly_and_collide's images in the sound driver's RPC send, waiting
for an IOP answer no one gives. That wait is now answered in `whole_frame`, and the tape's four wrecks
run on the capture's own line and are judged call by call: the port's wreck test agrees with the
original's on every call. Details in 1-fly-wrecks-whole-frame-FINAL.md.
