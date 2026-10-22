# 2 — rider rider collision (FINAL)

Task S78. 2026-10-22.

## What I did

S78 was reopened on 2026-10-22 with "do what can be done now; the parts that wait on another task go to that task". S86 (the collidables) and S93 (the split-screen RAM images) were done. S87 (wire the record into the ride) was open and unblocked, and no other slot was running it. I checked s1, s3, s4 and s5: they were on S103, P106, S104 and MD10. So I did S87's wiring here and left S88 (the capture acceptance) where it is. The plan note on S87 records the split.

- **`crates/game-sim/src/rider_contact.rs`** (new):
  - `collider(&phy.collisions, chassis mass)` builds the riding bike's collision object through the bike builder's five setters.
  - `gather(seats, dt)` is the tick's search. Every listed bike's body rays run against every other bike's frame through `world_search::search_nearby`, then the pair filter, then `BikeCol::strike`. Then `contact_search::collide` runs in player order. It returns one list a player, and those lists include the mirrored kind-1 contacts.
  - `Bump::resolve` is `contact::resolve_ridden` on the bike's chassis, with every other player's chassis as the other side.
  - Two Sim methods: `Sim::bike_contacts/bump/bumped` for an offline session, and `among/bump_among/bumped_among` plus `Sim::step_bumped` for online seats, where each seat is its own Sim.
- **`ride.rs`**: `Ride::frame` is now `frame_bumped(.., None)`. With a bump, its list is resolved at the substep's `CollisionBegin`, before the impact accumulator is unrotated into `g_peak`, as the original's ground step does. The chassis is the own side and is written back to `mvc`. Impulses happen only when `k == 0`.
- **`rider.rs`**:
  - `frame_among` → `frame_bumped(.., None)`.
  - New constant `Rider::bike_col` and `carries_bike_col`. Like the other constants, the snapshot doesn't send or walk them.
- **`lib.rs` `Sim::step_all`**: the contacts are gathered before any player is stepped. Each player is stepped with its own list, and afterwards the velocity and rates of the other chassis are written back.
- **`openmx` `main.rs`**: loads `Bike\Col\bikem.col`/`bikef.col` by full path. The path matters because `wreckbikef.col` also ends in `bikef.col`. Every rider carries the files. Online seats are clones of the first, so they carry them too.
- **`net/rides.rs`**: the seat loop gathers with `among` and steps each seat with `step_bumped`. A seat nobody sits in, or one flying the plane, is in no list.

## What the original does

- **The collision node.** It is the level's (vt `0x00454a90`). Its update is `./kit dis 0x0026f800 0x0026f900`. Its list (`+0x30` count, `+0x38` array) holds, in the S93 images:
  - the two parked biplanes' objects;
  - P1's `+0x7d0`, then P2's.
- **The update, once a tick:**
  1. Every object's vt+0x74 (clear).
  2. Every object's vt+0x7c (the search), in list order.
  3. The dynamics world's step.
  4. The scene tree's children.

  So every contact is found from where the last tick left every bike. Each bike's ground step then resolves its own list at `CollisionBegin`: vt+0x80, impulses on the first substep only.
- **The tree walks P2's vehicle node before P1's** (0x12e3b20, then 0xc1f860) in all three images.
- **The riding object's words** are identical for P1 and P2 in all three images:
  - search 1, ground 0, sweep back 1, damp 0;
  - mass `CollisionWeight·0.0310559` and its inverse; inverse inertia (0,0,0,1), because the chassis tensor is still zero when `set_mass` runs;
  - capacity 11, actors `0x2008`, collides `0xffffffff`.
- **The setters**, called by the bike builder at `0x255b50..0x255be0`:

  | Setter | Recipe | Writes |
  |---|---|---|
  | `set_mass` | `./kit dis 0x26fd80 0x26ff10` | the mass and inertia words |
  | `set_springs` | `./kit dis 0x26ff10 0x2701a0` | `|k·m·32.2|`, `d·2·√(m·k)` on the chassis' `+0x210`; body −1 copies |
  | angular flags | `0x2701a0` | |
  | moment masks | `0x2700d0` | the body vector −1 (`0x426d90`) copies |
  | friction | `0x2701d0` | body −1 copies |
  | per-surface friction | `0x2701b0` ×16 | |
  | high-speed sweep | `0x270220` | |
- **The ghost switch** (`./kit dis 0x00250580 0x00250600`, with vt+0x104 and vt+0x108 at `0x00250500` and `0x00250470`):
  - It takes the frame collidable out of the broad phase.
  - It adds the vehicle category to the ignore list of the rays and of the sweep.

  So a ghost neither strikes nor is struck.

## How it was proved

- **`tests/rider_contact.rs::the_collider_is_the_originals`.** Six records: the S93 images splitscreen, s93-short-5997 and s93-long-3402, each with P1 and P2. The test checks every word the resolution reads against `collider()` off the disc's `race125p.phy`, bit for bit:
  - mass and inverse, inverse inertia, centre;
  - actors, collides, the frame's category;
  - max depth, both springs and restitutions, both angular flags, the four masks;
  - both frictions and all 16 surfaces;
  - plus the chassis' `+0x210` = the file's mass.

  I confirmed the count of 6 by forcing the assert.
- **The port, on the stand-in bike with race125p's `[Collisions]`:**
  - **Side by side at 1.5 ft**, each list holds two struck contacts and two mirrored ones, all against the other rider. In 30 still ticks the gap opens from 1.5 to 4.39 ft. Without the files it stays at 1.49999 ft. Both numbers were read by forcing the assert.
  - **Determinism:** a clone, and an encode → ride 7 → decode, both ride the same 30 hashes as the first run.
  - **Ghosts:** either rider a ghost gives empty lists and the same hashes as a ride without the files.
  - **One rider** with the files hashes as one without, for 30 ticks.
  - **Online:** two one-player Sims, as online seats, gather the same lists as the two-player Sim, and their chassis after one tick equal its chassis.
- **The shell:** `openmx --headless --players 2 --frames 60` loads the files and runs 60 frames.

## Not verified / not 1:1

- **No capture of the original's bikes pushing apart.** Nothing compares the port's push tick by tick against the original (S88). Proved: the tuning words, `resolve_ridden` (entry 1), the ray-against-frame strike (S86) and the masks (S86). Not proved: the whole chain in motion.
- **Step order.** The S93 images walk P2's vehicle before P1's. The port steps in player order, as S54 read it. When P1 strikes P2, which chassis' resolution sees the other already moved therefore differs. Left for S88 to settle against a capture.
- **The broad phase's gather.** It is not the original's. The candidates are every other listed bike's frame, in player order, with distance = |rays' centre − frame's root centre| − frame reach. The original gathers through its tree and keeps stale candidates until the probe leaves its bound. The narrow phase's answers are the same; the order of contacts in a list may not be.
- **Rays only against bikes.** The riding bike's body rays against the world's props and the plane are not resolved here. The ride's own bundle still stands in for the props, and the bikes' frames are not in the plane's world beyond S95's list.
- **The chassis sweep back** (`+0x3c` = 1) is not ported: `contact_search::sweep_back` has the body case only.
- **What the other chassis keeps.** The others' shock/twist (`+0x170`/`+0x180`) additions are dropped. The port's ground step empties them at the top of the next frame, and nothing reads them in between in the port. That is not checked against the original's other readers.
- **A wrecked rider's object** is taken off the list (frame disabled, no search). That is inferred from the wreck swap, not read.
- **Wreck from a push.** A strong impulse lands in the impact accumulator, as in the original, so a hard enough side hit can trip the landing wreck test. Not exercised.
- **Online.** A seat nobody sits in is out of the list (the port's own rule, from the missing pad). S54's head-ray joust still does not run online.

## Next

- S87's wiring is done here, and I noted that on S87.
- S88: take a capture of the two riders pushing each other in the original, compare tick by tick, and settle the P2-first step order.
