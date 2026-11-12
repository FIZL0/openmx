# 1 — generic vehicle avoidance (FINAL)

Task S317. 2026-11-12.

## What I did

- **`ai::avoid_vehicle`** (crates/game-sim/src/ai.rs) is VehicleAI's per-tick avoidance, the pass
  every riding brain (mode 5) runs where the motocross one (`ai::avoid`) is not taken. Its parts:
  - `VehicleTuning`: the six vehicle tunables, with the constructor's defaults.
  - `Thing`, `Scene` and `VehicleScene`: the scene walk and the feeler sweep are callouts, like the
    path queries.
  - `blocked`: the reason bits.
  - `VehiclePass`: the fan, the per-feeler speeds, the reasons, and the feeler taken.
  - Two new flag bits, `CLIMB_CHECK` (0x2000000) and `DROP_CHECK` (0x4000000).
  - `VEHICLE_FAN_SPREAD` (π/2) and `BUNDLE_W`.
- **The S12e join runs it.** `Around::vehicle_avoid` (the stand-in hook) is gone, replaced by
  `Around::vehicle: Option<VehicleScene>`.
  - The ground probes ask `Around::world`.
  - The hit reaction writes `Around::recovery` and draws from the tick's generator. The generator is
    now shared between the avoid and trick-plan callouts through a `RefCell`.
  - The pass comes back as `Told::vehicle`.
  - With no scene given, the tick still tells `"vehicle avoid"`.
  - `Racer` gains `vehicle: VehicleTuning`.
- **Oracle** `vehicle_avoidance_matches_over_ten_thousand_passes` (crates/game-sim/tests/ai/avoid_vehicle.rs).
- **S12e's test** (tests/ai/sim.rs) drops the stand-in. On free09's three riders it now runs the
  port's own pass on the original's replayed ground answers, scene walk and sweep answers.
- **research/oracle.txt:**
  - an S317 note under `ai_update_vehicle`;
  - new names: `class_bit` (0x00119520), `ai_iface_walk_begin` and `ai_iface_walk_next` (the
    interface's vt+0x2c/+0x30 thunks).

## What the original does

`./kit fn 0x002858d0` (ai_update_vehicle). It takes the same arguments as the MX update:

| Register | Argument |
|---|---|
| a0 | the brain |
| a1/a2 | the ignore count and list |
| a3 | the position |
| t0 | the heading |
| t1 | the steer (in/out) |
| t2 | the speed |
| t3 | the aimed-for speed (out) |

The S12e call site aliases t0 = t1 and t2 = t3.

1. **Early return.** It returns at once when the interface (+0x40) is null or the speed is `<= 0` (a
   NaN goes on).
2. **The bundle.** It allocates the bundle (+0x2a0) on first use. The builder sets every direction's
   and origin's w to 1.0.
3. **The fan.** n = +0x2a4 feelers. It is the same code as the MX fan with the spread doubled:
   angle π/2·(0.5 + i − 0.5n)/n. Each feeler gets:
   - origin = the position with y + height (+0x2a8);
   - length +0x2ac;
   - speed[i] = the speed handed in.

   The bundle's centre (+0x150) is the position.
4. **The scene walk.** It runs when `(flags&0x80)==0 || (flags&0x40)==0`, via the interface's
   vt+0x2c/+0x30/+0x34. It first clears the bundle record's arrays.
   - **Type-2 objects** are swept with bundle_sweep_object (`./kit fn 0x0011c210`) when all of these
     hold:
     - the object is collidable (+0x31 & 1);
     - it is not in the ignore list;
     - its +0x84 lacks the AI_IGNORE class bit (`./kit fn 0x00119520` on "AI_IGNORE");
     - 0x80 is clear.

     If the sweep meets the object, every feeler whose hit flag is set (record +0xa4), or whose
     neighbour's is, gets speed 0 and reason 2.
   - **Type-3 placed groups** are looked at when `(flags&0xc0)==0`. The point is
     (u16 +8, u16 +0xc) × world_manager+0xd4. A feeler whose ray passes within +0x280 of the point,
     inside its length, gets speed 0 and reason 4. group_part_count is called first, but its result
     is never read.
5. **The riders.** It walks the race's rider list, skipping the own card and inactive riders. There
   is no SKIP_MARKED test here, unlike the MX pass. For a rider ahead along a feeler, within +0x284,
   and within +0x280 of the ray:
   - the feeler's speed becomes min(speed, dot(dir, rider velocity));
   - if that closing speed is ≤ 0, it also gets reason 1.
6. **The path test.** With flag 0x20000, a track, and a route node with two nexts, the point +0x288
   ahead along each feeler is tested against two segment pairs. If it is in neither, the feeler gets
   speed 0 and reason 8.
7. **The ground probes.** Each one asks the interface's vt+0x38, which is ground_query on the
   rider's world, at +0x300 ahead along each feeler.
   - **Climb** (0x2000000): rise/+0x300 ≥ +0x308 while the held speed +0x80 < +0x30c gives reason
     0x10.
   - **Drop** (0x4000000): otherwise, rise/+0x304 ≤ +0x310 gives reason 0x20.

   Either one sets the feeler's speed to 0.
8. **The choice.**
   - The middle feeler (n/2) starts as the best, with worth = speed·dot(dir, heading).
   - Walking all of them in order, a feeler takes over only when its worth is strictly higher and
     held·sin(angle to the steer handed in) < +0x328.
   - If the best worth is above 0, the steer becomes the feeler's whole quad (w 1), the aimed-for
     speed becomes its speed, and 0x100 is cleared.
   - Otherwise the interface's vt+0x3c is called: ai_hit_reaction (`./kit fn 0x002812b0`, already
     `ai::hit_reaction`) with the reason bits.
9. **Flag 0x200** draws the debug fan. Nothing else reads it.

free09's three brains:
- n = 11, flags 0x3e0b82b / 0xbe0b82b. So the sweep and the groups run, the climb probe runs, the
  drop and path tests do not.
- Tunings 5 / 100 / 10 / 3 / 100, then the vehicle defaults 20 / 20 / 0.5 / 40 / −0.5 / 150.

## How it was proved

**Function oracle, 10,000 generated passes** (tests/ai/avoid_vehicle.rs,
`vehicle_avoidance_matches_over_ten_thousand_passes`). Each one runs the whole original
`ai_update_vehicle` in Ee.

- **Hooked:**
  - the interface vtable: walk begin/next/end, the ground (scripted heights written into p.y), and
    vt+0x3c, which is the real ai_hit_reaction on the real generator;
  - bundle_sweep_object (scripted met/hit flags, the same ones handed to the port's `Scene::sweep`);
  - group_part_count (returns 0);
  - class_bit (a fixed mask);
  - path_segment_holds (scripted).
- **Generated:**
  - every flag bit;
  - the tunables (a quarter of passes use free09's numbers);
  - 1–16 feelers;
  - riders along the fan, the own card sometimes among them;
  - scene objects of types 1/2/3 with an ignore list;
  - negative speeds;
  - the S12e aliasing of t0/t1 and t2/t3 in a third of passes.
- **Compared:** the steer (4 lanes), the aimed-for speed, the flags, the generator, the three recovery
  words, the whole fan and the bundle's centre, the objects swept, the groups taken in, and every
  ground and segment point asked.
- **One run's counts:**

  | Outcome | Passes |
  |---|---|
  | took a feeler | 8239 |
  | turned off the middle feeler | 3018 |
  | hit reaction | 1258 |
  | recovery changed | 645 |
  | idle | 503 |
  | reason rider | 3588 |
  | reason solid | 981 |
  | reason group | 706 |
  | reason off path | 1552 |
  | reason climb | 1442 |
  | reason drop | 3420 |

  Every pass matched.
- **Mutant:** dropping the right-neighbour spread of a sweep hit fails at case 17.

**S12e on free09** (tests/ai/sim.rs). Three riders × 60 ticks now run the port's own pass:
- The original's 13 ground answers per tick (2 from the feed, 11 probes) are replayed in order, and
  every asked point is compared.
- The scene walk's objects (read off each one as the original's walk handed it out) and its sweep
  answers are replayed. That is 28,080 objects walked and 16,020 sweeps. The swept list, the fan in
  the brain's bundle and the recovery words are compared, along with everything S12e already compared.
- The reasons seen are 0 (120 ticks) and 1 (60 ticks: a rider closing on a feeler). Every tick took a
  feeler, and 6 ticks drew from the generator. Both counts were read from a one-off `panic!` probe.

## Not verified / not 1:1

- **The scene walk and bundle_sweep_object are not ported.** They are the caller's (`Scene`). In the
  oracle both are scripted. In S12e they are the original's answers, replayed. Until the Sim has a
  spatial query and a ray-against-object sweep, a Sim rider has no scene. With `vehicle: None` the
  tick tells "vehicle avoid".
- **The AI_IGNORE class bit is handed in** (`Thing::Solid::ai_ignore`). The class registry is not
  ported.
- **Fewer than one feeler is not supported.** With n = 0 the original reads an uninitialised stack
  slot for the middle feeler. No shipped brain has n = 0 (the constructors set 11).
- **The bundle record's arrays are not modelled.** The original clears them and lets the sweep fill
  them. The port passes per-feeler hits straight from `Scene::sweep`.
- **The debug draw (0x200) and group_part_count are left out.** Neither changes an output.
- **The path test is a closure** (`VehicleScene::off_path`), as in S12c/S282. The oracle scripts its
  answers. S12e's riders do not set 0x20000.
- **The interface's thunks are hooked or replaced in both tests.** The oracle points vt+0x3c straight
  at ai_hit_reaction, with the scene as the rider base. In-game it goes through the 0x10d0 thunk. The
  thunk only moves a0, read by eye from `./kit dis 0x00281b50 0x00281ba0`.
- **No AI rider rides in the Sim yet.** The bodies on this steer are still not compared against a
  capture (S12e's next step).
- opentennis has no rider AI. The PC exe was not read; the PS2 code was enough.

## Next

Give the Sim its AI lane (S12e's next): a `Racer` from the roster's `Character`, a bike per rider,
`Racer::tick` before each AI bike's step, and a `Scene` for the Sim. The walk is the broad-phase tree
query, and the sweep is the ray-bundle-against-object test. Then record the free09 4-racer capture.
