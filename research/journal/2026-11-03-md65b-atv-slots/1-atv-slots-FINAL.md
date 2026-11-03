# 1 — atv slots (FINAL)

Task MD65b. 2026-11-03.

## How they were paired

The two vtables are not laid out one slot per entry on both sides, so a slot's position alone does not
pair it. I used anchors instead: the slots already paired (vf46 = +0x378, vf55 = +0x3a8, vf70 = +0x3f0,
vf84..87 = +0x46c..+0x478, vf90 = +0x49c, vf110 = +0x550). From those, I read each candidate in the
quad's vtable 0x003ce250 (`./kit data 3ce250 … -p atv`) and matched it to the PC body by three things:

- what the body does to fields (the PC's are 0x20 higher: wheels +0x17d0.. against +0x17b0..);
- the callees it calls, and in what order;
- the strings it uses.

| PC slot | PC | PS2 | vt | what | evidence |
|---|---|---|---|---|---|
| 0 | 0x005d7550 | 0x004b89a0 | +0x8 | destructor | vptr resets, base dtor |
| 33 | 0x005d61d0 | 0x004b8ab0 | +0x340 | surface remap after the level load | level setup, then a 16 × 4-wheel copy loop |
| 35 | 0x005d9020 | 0x004bb710 | +0x348 | sound/particle/effect listeners wired | callees |
| 38 | 0x005d6260 | 0x004b8b80 | +0x354 | wheels' field pointers into a block | +0x788 (PS2 +0xb94), four wheels |
| 39 | 0x005d6d90 | 0x004bb360 | +0x358 | LR wheel's surface byte | +0x17d8/+0x110 against +0x17b8/+0x110 |
| 67 | 0x005db3a0 | 0x004be3c0 | +0x3e4 | animation driver | biplane base first, vf111 later, hung-node loop |
| 80 | 0x005f6570 | 0x004be3b0 | +0x41c | LF strut travel | +0x3c; shared with the bike on the PC |
| 86 | 0x005d7bf0 | 0x004ba510 | +0x474 | trigger probe's segments | +0x1450/+0x1430 guard, 8 points, shape set |
| 104 | 0x005d8bb0 | 0x004bb400 | +0x4f8 | wheels settled on the ground | turn, node add, ground query ×4, +0x140→+0x120 |
| 111 | 0x005dc4f0 | 0x004be8d0 | +0x554 | model node pose | called from vf67 on both sides |
| vf3_at_1084 | 0x005dd370 | 0x004bb370 | +0x558 | model reload | attach nodes, node finder, put-back(1, 0) |

The test `the_quads_md65b_slots_hold_the_paired_functions` pins the PS2 side of every row. The
PC's side was read with `./kit fn … -o pc`.

## What is ported

- **`remap_surfaces` (slot 33).** First the level setup, which is a `Call::LevelSetup`. The original sets
  a global flag and then calls MotoShared's level setup. Then, for each of the 16 surfaces the level
  maps, the wheel's surface copy runs on LF, RF, LR and RR.
  - The copy carries 23 of the 24 tables. It skips +0x738, which is `side[12]`, the
    `DonutsSideForceSpeedScale`. `the_quads_surface_copy_keeps_the_donut_scale` shows this on the
    original's copy alone.
  - The copy runs in place, so chains propagate: a surface mapped to one that was already remapped
    takes the remapped tables.
  - Ghidra drops LF's third argument; the asm hands `map[i]` to all four calls.
- **`surface_under` (slot 39).** The LR wheel's surface byte. **`strut_travel` (slot 80).** The LF
  wheel's +0x3c.
- **`trigger_segments` (slot 86).** Per wheel, mount.y + strut travel (`add.s`) and mount.y − radius
  (`sub.s`); the other lanes are the mount's. It returns `None` when there is no trigger probe. The
  original hands the eight points to `shape_set_segment` (count 4, flag 0, the scene node). That call
  is a side effect the caller makes; the port returns the points.
- **`settle_wheels` (slot 104).** Per wheel, in order:
  1. mount + probe on xyz;
  2. turn it into the world with `Body::to_world`, the same two VU0 rounds;
  3. add the scene node's position;
  4. run the ground query.

  After all four, each wheel copies the point into its kept point. Ghidra stops the function at the
  first query; the asm runs to 0x004bb710. The query is a closure the caller supplies, so the world's
  ground query (atv_ground_query, not ported here) stays outside the port.

## Checks

`./kit check -- -p game-sim --test atv_quad`: 45 passed (39 before, plus 6 new). I ran the new six
with `cargo test … -- --nocapture` and none skipped.

| test | cases | paths |
|---|---|---|
| remap | 10,000 | unmapped / past the surfaces / onto itself / from a remapped surface / copied, each ≥ 1,000; checks every table word of the four wheels and the call log |
| answers | 10,000 | |
| trigger | 10,000 | with a probe and without, each ≥ 1,000; segments compared bit for bit |
| settle | 10,000 | all hit / a wheel missed, each ≥ 1,000; the points queried, then every field of the four wheels bit for bit |

Each test also checks that the original wrote nowhere outside the wheels (and the flag word).

I made eight hand mutations, and each turned its test red:

- copying the donut table;
- remapping in reverse order;
- LR → RR;
- LF → RF;
- add → sub in the trigger segments;
- no node add;
- a full-lane add for mount + probe;
- no keep loop.

A ninth, keeping the point before the query, was a no-op, because the keep loop overwrites it.

## Not verified / not 1:1

- **No caller wiring.** Nothing in the ride calls the five ports yet. The settle's ground query and the
  trigger's shape set are left to the caller.
- **Driven only by generated cases on the spawn image.** No capture was taken of a level load's
  remap or of a settle in play. The level's actual surface map at +0x140 was not read from a real
  level.
- **The donut skip on the PC was not checked.** The PC's copy (0x0046fa30) was not read word for word,
  so I have not confirmed it skips the same table.
- **The surface byte is a byte.** The ground query's `sb` to +0x110 was read from Ghidra's
  `*(undefined1 *)param_5`. The rest of that query is not ported.
- **Slots 0, 35, 38, 111 and the reload are paired but not ported.** None of them is simulation: the
  destructor, listener wiring, field pointer wiring, the model's pose and the reload. Their bodies are
  summarised only in oracle.txt.
- **Slot 67 (the animation driver) is not ported.** It writes +0x1524 and +0x141c, which the sim reads.
  Filed as MD65c.
