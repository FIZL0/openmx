# 1 — the base driver and its net check (FINAL)

Task MD65c1. 2026-11-03.

## Read

- **PS2:** I read `./kit dis -p atv 4a8350 --len 0x80` (the base) and `./kit dis -p atv 4a86f0 --len 0xd0`
  (vt +0x3e8). The quad's vtable 0x003ce250 has these slots: +0x3e4 animate, +0x3e8 0x004a86f0 (the
  biplane's own), +0x3ec 0x004e7bc0 (the quad's own), +0x3f0 the net unpack.
- **PC:** MXVehicleBiplane::vf67 has the same shape. In states 3 and 4 it copies the (0,0,0,1)
  constant into +0x340, calls vt +0x110, and calls +0x118 when that answers. PC hands dt to both.
- **What vt +0x3e8 asks:** the brief guessed "is remote", and it is not that. It is the remote's
  snapshot interpolation, and it returns 1 on both of its paths (two `addiu v0, zero, 1`). So in
  vehicle states 3 and 4 the net unpack always runs.
  - With fewer than five snapshots (+0xd04) it does nothing else.
  - Otherwise it takes want = now − delay − 0.1 and walks the five snapshot slots (+0xcd4..+0xcf4,
    8 apart, newest first). It calls vt +0x3ec with the pair it found and t clamped to −0.9..1.9.
  - The walk has a quirk: its first step reloads slot 1, so it compares slot 1 twice and never
    compares slot 4's time. The pairs that come out are (1,0), (2,1), (3,2) and (4,3).

## Ported

`quad::base_animate(vehicle_state, &NetClock, &mut Quad, &mut Body, dt, &mut impl Moto)`:

- Outside vehicle states 3 and 4 it does nothing.
- In those states:
  - `Quad::world_ang_vel` is set to (0,0,0,1).
  - With five or more snapshots, the walk above runs and gives `Call::NetInterpolate { older, newer, t }`.
    `older` and `newer` are places in `NetClock::times`.
  - Then `Call::NetUnpack(dt)` follows. Whoever runs the calls answers it with `quad::net_unpack`.
    The original reaches the unpack through the vtable, so it is a call here too, like the node pose.

`quad::animate` takes a `&NetClock`, and `Call::BaseAnimate` is gone.

## Checked

`cargo test --release -p game-sim --test atv_quad animate` passes. Both animate tests now run the
base driver and the net check unhooked in the emulator. Only the quad's interpolation (vt +0x3ec) and
the net unpack are hooked, logged with their pointer and float arguments.

- **The generated test:** 20,000 cases. Each one lays the network clock's time (through the clock's
  pointer), the snapshot count, the lag, five snapshot records (the step between them is sometimes 0,
  so the walk divides by zero) and +0x340.
- **What is compared:** the calls, and the world angular velocity quadword by bits next to the
  earlier five words.
- **Every path at least 200 times:** not held, too few snapshots, each of the four pairs, and t
  clamped low, clamped high and inside.
- **The spawned quad:** the image has vehicle state 1 and 0 snapshots, so in state 3 the base takes the
  "too few" path there.
- **Four hand mutations, each red:**
  - the walk without the slot-1 reread;
  - the 0.1 lag set to 0;
  - the angular velocity not zeroed;
  - the snapshot threshold changed to 4.

`./kit audit` shows only the 4 leaks already on main.

## Not verified

- The interpolation itself (vt +0x3ec, the quad's own) is not ported. It is a `Call`, filed as MD65c2.
- No live capture of a remote quad. The clock at 0x0038f768 is named from its use here only.
