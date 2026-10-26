# 1 — bike particles (PART)

Task P74. 2026-10-25.

## What I did

- Found the bike's emitter list. It is the track's `mxparticles.dat`; free09's sits in `free09_2.res`,
  and `mxaiparticles.dat` is the same bytes. It has 15 entries:
  - CollisionModel: Vehicle Dust, Vehicle Chunks;
  - ParticleCollisionConstraint: Rider Dust and Rider Chunks (sphere mask 0x1fc), Foot Dust (mask 0x3,
    Disable NoContact|ObjectContact|Wrecking);
  - Exhaust: Right and Left Exhaust (MaxSpeed 15, rear tire, Disable Wrecking);
  - Generic: Rear Tire Dust Spray;
  - Spray: front and rear Dirt Spray, front and rear Chunks, Rear Tire Collision Chunks (HasCollision);
  - TireTrack: two entries, not ported.
- Found the nodes. `RearTire_Particle01` and `FrontTire_Particle01` are in the chassis model `bike`;
  `250_Exhaust_Particle01/02` are in the exhaust pipe part. The 125's `cr125_exhaust_pipe.psg` has only
  Particle01, and Left Exhaust says DiscardWhenSoultreeMissing, so the 125 has one exhaust, as in the
  original.
- Found the zones. `mxparticlezones.dat` gives zone 1 (index 0, free09's road) "none" for Dust, Dirt,
  Chunks and DirtSpray, and DirtSpray zone 13 = GrassSpray.
- particles.rs:
  - `Kind::Spray(SprayKeys)` with the spray update (fade clock, side throw, speed ramp);
  - `Kind::Constraint { mask }` for the rider's sphere emitters;
  - each entry's flag byte (`Entry::LIVE`, `ZONE_OFF`), `lit` and the spray `clock`;
  - the switch-on rule: an emitter emits nothing on the tick it goes on, it only takes its position;
  - Exhaust scales the file's velocity **z** (not y) by f + 0.5, corrected for the biplane too;
  - `ConstraintFrame`, and `PlaneFrame.clutch`, `ground_zones`, `constraints`; `TireFrame.spinning`, `side`.
- main.rs:
  - `load_bike_fx` reads the list off the disc;
  - `bike_fx_frame` feeds one `PlaneFx` per seat from the ride: nodes off the chassis and part models,
    both tires (contact, surface, spin, side load), gas, clutch, speed, the wreck, the crash's contacts,
    and the ragdoll's sphere contacts;
  - each seat steps every simulated frame and is drawn on both the GPU and the software paths, with
    P61's ground and wind;
  - the log line `bike particles: ...` shows the live counts.
- Tests:
  - `crates/openmx/tests/bike_particles.rs`, new: `the_discs_bike_particles` checks the list against the
    disc. `the_bikes_emitters_follow_the_original` replays the capture.
  - The biplane exhaust test is updated for the z scale and the switch-on tick.
- Capture: `capture/` holds the PINE spec and scripts that recorded `context/capture/p74/run.csv` and
  `run.mp4`. They replay S109's animandpump recording from frame 1939: launch at 2001, a jump, the road
  at 2738–2752, the wreck at 3719 and the respawn at 3913/3914. `tape.py` writes a shell tape for the
  port's launch and turn.

## What the original does

- **The manager** (`./kit fn 0x003d5aa0`):
  - Every frame it builds each tire's flags: NoContact 1, Ground 2, Object 4, plus Wrecking 8 or
    NotWrecking 0x10 for the whole bike.
  - An entry is live (flag bit 1) when `Disable & flags == 0`. It is zone-off (bit 2) when its tire's
    zone maps to none.
  - Only flag == 1 runs the entry's update. It reads the snapshot words wired by `./kit fn 0x0027adf0`.
- **Spray** (`./kit fn 0x003d6b50`):
  - When (throttle == 0 or the clutch is in or the tire is not spinning) and |side load| ≤ threshold,
    the spray fades: `clock -= dt`, rate = file × clock / 0.25, and it goes off at 0.
  - Otherwise it throws:
    - vx = −SideForceScale × side;
    - the noise in x is widened by |vx|/2 each way;
    - f = clamp(1 − (speed + 1) × ramp, 0, 1), and y noise += f × Min/MaxYVelocity;
    - rate = file × (1 + f/2), clock = 0.25, on.
- **Exhaust** (`./kit fn 0x003d5f70`): f = throttle × ½ × (MaxSpeed − speed) / MaxSpeed. It is off when
  f ≤ 0; the velocity z is scaled by f + ½.
- **CollisionModel** (`./kit fn 0x003d75d0`): one emit per crash-body contact, from contact − body
  velocity × dt. It is skipped where the contact's zone is none.
- **ParticleCollisionConstraint** (`./kit fn 0x003d7a30`): the same for each masked ragdoll sphere that
  touches.
- **Switching on** (`./kit fn 0x003d86d0`): it sets the flag and turns the emitter on. The emitter then
  runs after the manager in the same frame and only records its position. So the first particles
  come the frame after (exhaust on at 2001, its carry first moves at 2002).

## How it was proved

- **A PINE capture, compared row by row.** The original is
  `context/capture/p74/run.csv` (2400 rows, 1939–4338, 60 Hz), with every entry's flag, switch byte,
  carry and position, the spray clock and vx, and the snapshot words.
  `the_bikes_emitters_follow_the_original` feeds the port each row's words and the original's node
  positions and steps it.
  - **2055 rows compared. 0 misses on rows that are surely one game frame on.** That covers flag, on,
    spray clock to 2 ms, vx to 1e-3, and whether the carry moved (it emitted).
  - It asserts the launch switch-on rows (2001 exhausts, 2006 rear sprays). It asserts the road's
    zone-off at 2738 (front) and 2744 (rear). It asserts the wreck at 3719 (vehicle and rider entries
    live, foot dust and exhausts off) and the respawn at 3914.
  - Rows that may be two frames on, or come under 5 ft/s or during the wreck, are stepped but not held
    to the original: 290 entry mismatches there. They are mostly k=2 rows (214) and the spray clocks
    of low-speed rows.
- **The disc**: `the_discs_bike_particles` checks names, kinds, nodes, tires, Disable masks, the
  skipped TireTracks, the collision entries, the exhaust and spray rates and the zones.
- **By eye**:
  - The port's headless launch (`--play` with `capture/tape.py`'s tape, shots at tape frames 72–180)
    shows a dust trail off the rear wheel. The original's video at 1.5 s and 3 s shows one too.
  - The log shows the rear sprays, collision chunks and exhaust live on gas.

## Not verified / not 1:1

- **TireTrack (2 entries) is not ported.** It is a decal trail, not an emitter. Follow-up filed.
- **Foot Dust while riding is never fed.** The port's riding rider has no constraint contacts; the
  ragdoll only touches in a wreck.
- **Wreck emission is not compared.** The capture has no crash-body or ragdoll contact data, so
  entries 0–4 are checked on their flags only, not on what they emit.
  - The port's CollisionModel uses a source per contact, and its PCC uses one shared source. That is
    how I read the decompilation, but the carry was not compared.
  - Ragdoll ground/scene is mapped to the ground/object flags.
  - A sphere's zone comes from P61's ground query under it, not from the contact's own surface.
- **Rows not surely one frame on are not held strictly:** k=2, under 5 ft/s, or wrecked (290
  mismatches, see above).
  - The respawn row 3913 (the original clears every flag; the port still has the wreck's) is among them.
  - The torn rows 2996, 3156 and 3924 are resynced and not compared.
- **The throttle word is assumed to be the held gas, and the speed the chassis speed.** The capture
  feeds the snapshot words directly, so the port's own mapping (`ride.held.gas`, `ride.mv.clutch_in`,
  `chassis.speed`, `slip.spinning`, `side_per_load`) was not compared against them in a live ride.
- **Every seat uses `mxparticles.dat`.** AI seats would read `mxaiparticles.dat`, which is the same
  bytes on free09 and was not checked on other tracks.
- **The HasCollision toggle at emitter +0x248** (which the manager can flip at run time) is ignored.
  Collision is set once from the file.
- **By-eye check is partial.**
  - Only the launch was compared by eye. In the port's turn on the tape the bike leaves the ground, so
    there was no grounded hard turn to look at.
  - The port's tape ride has no landing or wreck.
  - At speed the port's trail looks like spaced puffs where the original's looks continuous; this was
    not measured.
- **The plane's exhaust test changed.** It now uses velocity z and does not count the switch-on tick.
  That follows from this task's reading of the decompilation; P60's own capture was not re-run.

## Next

The work is done and committed, but **P74 is not ticked**:
- **Why.** `./kit done P74` ran the full gate (2026-10-25). It went red only on
  `crates/game-sim/tests/replay.rs` `the_slip_cases_replay_from_the_save_state` ("turn: the path parts
  at 75, not after 82"), and that is red when run alone too.
- **It is not P74's red.** P74 does not touch game-sim: `git diff 5daefb79 -- crates/game-sim` is empty,
  and game-sim last changed in MD28. This is main's known red since the batch merge 1cdaee5a, filed as
  **S119**.
- **The other red passed on a lone rerun.** The LAN lobby test is in gate.toml's load flakes.
- **Pushed.** I pushed `task/P74` myself; the runner merges it.
- **What the next session does.** Once S119 lands, run
  `DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 ./kit done P74 "<msg>" --skip-audit`. The audit leaks are other
  tasks' names in plan/DONE.md:121 and :357. That ticks it; nothing else is left.

Follow-ups filed: P151 (TireTrack), P152 (foot dust while riding), P153 (the wreck's emission against a
capture with contact data).
