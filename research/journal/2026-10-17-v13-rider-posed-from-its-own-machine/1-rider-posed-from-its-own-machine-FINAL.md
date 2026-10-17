# 1 — rider posed from its own machine (FINAL)

Task V13. 2026-10-17.

## What I did

V12's capture found the renderer's two animation machines paired with the wrong skeletons. This task
unpicks the pairing in `game-render` and takes the seven numbers V12 measured at the same time.

1. **`crates/game-render/src/anim.rs` — new `anim::Stance`.** The sixty-state machine's rig: a
   `Set`, the `vehicle_anim::Library` table, the `Driver`, the motion clock, the pose snapshot and
   the `Params`, with `frame(dt, want, &Request)` running one step of the machine over the set.
   It is `anim::Rig`'s sibling — `Rig` is the 28-slot `game_sim::motion` machine, `Stance` is the
   60-state `game_sim::vehicle_anim` one — and the doc comment carries the table of which machine
   poses which skeleton so the next reader cannot cross them again.

   `Stance::bound` starts the driver the way the body's constructor does rather than zeroed: state,
   latched state and previous state all `TURN_RIDING` (`0x1e`), `extra` at `-1` (negative is "no
   added record"; a zeroed `i32` asks for record 0 on the first frame), and both eased axes centred
   at 0.5 with the time constants off the build — `MIX_TAU` 0.125 and `PHASE_TAU` 0.15.

   `Stance::new` is the body's loader: the set's own `[MOTION n]` list, then the 24 gesture slots as
   `<prefix><stunt>` with no `UsedForBike` gate, then the 36 pose names bare — and, for a set whose
   `.mcf` names its motions with the actor's word in front (every bike's does: `NAME=bike_air_Mid`),
   a second ask for each still-empty pose slot with the prefix on. That retry is V9's and is still
   the port's own reading of a seam, not a re-read of the loader.

   `Stance::posed` ("a clip has written a pose at some point") gained a sibling `Stance::drawn`
   ("the *last* frame wrote one"). A frame that comes back `false` is a row of the table the set does
   not have, which is how a set paired with the wrong machine shows up; it is what the second test
   below counts.

2. **`crates/game-render/src/vehicle.rs` — `Chassis` holds a `Stance` instead of being one.** All
   the machine's fields moved behind `chassis.stance`; `Chassis` keeps what is the *bike's* and only
   the bike's — the five simulation-driven node drives, the two wheel-roll integrals, `Geometry`,
   `Nodes`, the composed `world`. Nothing about the drives changed. The point of the move is that
   the machine can now be paired with any set, which is what the tests pair it with the rider's.

3. **The measured `Params`.** `vehicle::params()` is no longer V9's five-threes-and-a-quarter guess:

   | field | V9 guessed | now (V12, measured over 1500 frames) |
   | --- | --- | --- |
   | `rate_ready` | 3.0 | **0.25** |
   | `rate_hold` | 3.0 | **0.25** |
   | `rate_default` | 3.0 | **4.0** |
   | `rate_pose` | 3.0 | **3.25** |
   | `rate_stunt` | 3.0 | **4.0** |
   | `cross_cap` | 0.25 | **1.0** |
   | `band` | 0, 0, 0 | **7.3333, 14.6666, 21.9999** |

   `start_phase` stays zero: V12 says explicitly that `+0x8f8` is *not* it, so it is still unmeasured.

4. **Three tests in `crates/game-render/tests/vehicle.rs`**, below, and V9's four still pass
   unchanged in what they assert (only their field paths moved under `.stance`).

## What the original does

The sixty-state machine is the **rider's body**, not the bike's. The object `./kit fn 0x002492b0`
(the step S25 ported) runs on has 22 joints, reads `Rider\Anim\` out of its content-directory buffer
and binds all sixty of its clip-table slots — the 24 `rider_<Stunt>` gestures and the 36 bare pose
names. The chooser in front of it is `./kit fn 0x00248a70` (ported in V11 as `vehicle_anim::Update`).

The bike's own object is the other machine, and its clip loader is `./kit fn 0x00258790`: two passes
over the 24 stunt records building `"<prefix>bike_<Name>"` into a 32-byte buffer, the first pass
gated on each record's `UsedForBike`, then `bike_air_Fwd`, `bike_air_Mid` and `bike_air_Bck` into
three named fields of their own. Those are the 12 clips V9 found on a bike archive — they are
*this* loader's names, which is why none of them answer a bare `turn_riding_Fwd`.

The numbers, from the constructor and the build:

- `./kit fn 0x002472c0` — the body's constructor. The three pose-grid thresholds are `7.3333`,
  `14.6666`, `21.9999` with a still band of `1.46666` and two more the step never reads (`73.333`,
  `80.6666`); `1/1.46666 = 0.681818` is the same constant the shell turns chassis speed into mph
  with, so **the band is feet per second** and the six thresholds are exactly 1, 5, 10, 15, 50 and
  55 mph. The idle countdown's range is 15 to 45 seconds. The state, the latched state and the
  previous state are all written `0x1e` — the body starts in the pose grid. The five fade rates and
  the cross-fade cap are *zero* here and written by something not located; V12 measured them off the
  running game instead.
- `./kit fn 0x00247a60` — the build, which makes the two lag records the axes are eased in: five
  words each, `{value 0.5, tau, 1.0, FLT_MAX, -FLT_MAX}`, with `0.125` on the mix axis and `0.15`
  on the phase axis, both starting centred at a half — which is what makes a body nobody is steering
  sit in the middle of each row of the grid.

## How it was proved

`./kit check` over the whole workspace: **passed**, including V9's four chassis tests with nothing
changed in what they assert. Three new tests in `crates/game-render/tests/vehicle.rs`, each printing
its numbers:

**1 — `the_sixty_state_tables_clips_are_the_riders`.** V12's capture restated as a test, off the
disc. The rider's set (`thq1.res` → `mrider.mcf`) under `Stance::new` with the `rider_` prefix:

```
the sixty-state table: 60 of 60 slots on the rider's 22 joints out of "Rider\\Anim\\";
  10 of 60 on the bike's 22 joints out of "Bike\\Anim\\" —
  0:bike_Taunt 3:bike_catwalk 6:bike_nacnac 7:bike_flatliner 12:bike_kod 16:bike_tsunami
  22:bike_supercanindianair 35:bike_air_Fwd 36:bike_air_Mid 37:bike_air_Bck
the bike's set holds 12 clips: bike_air_Fwd bike_air_Mid bike_air_Bck bike_air_Fwd_return
  bike_air_Bck_return bike_Taunt bike_catwalk bike_nacnac bike_flatliner bike_kod bike_tsunami
  bike_supercanindianair
```

Slot by slot it also checks the loader's split — prefixed stunt names in the 24 gesture slots, bare
`POSE_CLIPS` names after them — and that every bound clip has channels that land on joints of the
skeleton it was bound to, so a clip for another skeleton cannot pass. And the other side of the
swap: the nine rows of the grounded grid are bound on the rider's set and bound on **no** slot of the
bike's, which is V9's "the grid drew nothing" from the inside.

Two measured corrections to the record here. The 250sx answers for **7** of the 24 gesture slots,
not V9's nine; of the 12 clips its set holds, 10 are named by a state of the sixty and two
(`bike_air_Fwd_return`, `bike_air_Bck_return`) are listed in the `.mcf` and named by no state. And
a chassis skeleton on this disc has 22 nodes too, so the **joint count alone does not separate the
two objects** — what separates them is the content directory and that all sixty slots bind.

**2 — `a_minute_of_riding_poses_the_riders_own_skeleton`.** V9's own minute (the stunt on the way
down, then hands on), with the rider's stance and the bike's chassis stepped from the *same* wants
and the same request block, so the only difference between them is which set is on the other end:

```
a minute on the rider: 3600 frames, states {0, 30, 35}, blend modes free/one/two/_/four/pair
  [0, 79, 2, 0, 3519, 0], 0 frames with nothing to draw on the rider's set and 3519 on the bike's,
  fastest band 5.302 ft/s
```

Every frame: 22 finite transforms, one per joint; `drawn` true on all 3600 of them on the rider's
set and false on 3519 of them on the bike's; and the body is in a different pose at the end of the
minute than at the start (the furthest joint has moved).

That histogram also corrects V9's reading of its own minute. V9 recorded `[0, 79, 3521, 0, 0, 0]` and
read the 3521 as the airborne row; it is 2 airborne frames and 3519 frames of the **grounded grid**,
which with V9's zeroed bands came out as the grid's single-pair arm and with the measured bands comes
out as its interpolating one (mode 4). Nothing about the bike's drawing changed with the new numbers
— the grid is empty on a bike either way — but the entry's explanation of where that minute went was
wrong.

**3 — `the_grounded_pose_grid_draws_across_every_band`.** The fixture above never leaves the lowest
band (5.302 of 7.3333 ft/s, i.e. 3.6 mph), so the band and the mix axis are driven directly through
the request block instead of ridden: eight speeds, two in each of the four regions the thresholds cut,
times three positions of the fore/aft axis, half a second each.

```
the grounded grid on the rider: 24 settings drawn, blend modes {2, 4}, the furthest joint moves
  1.485 across a row and 1.144 between the first band's row and the third's
```

All 24 settings find their clips, both arms of the grid run, and the grid moves the body — across a
row and between rows. On a bike's set every one of these settings draws nothing.

**The 1:1 check behind all of this is V12/V15's**, and it is the reason this task is a re-pairing and
not a new measurement: `crates/game-sim/tests/motion.rs` compares the port's posed rider against the
original's own 22-joint array out of `context/capture/v12/rider-joints.csv`, and it does it through
the renderer's samplers with the **rider's** set loaded by the **vehicle's** loader — the pairing
this task installs. That capture is not in this worktree by default; I copied it in from the main
checkout and ran it: `./kit check -p=game-sim --test=motion` → passed, 146 of 1500 captured ticks are
one clip and nothing else, 22 joints each, every lane within 1e-4, worst `2.16e-5` on joint 3
(`UprLegR`)'s quaternion during `rider_lazyboy`.

## Not verified / not 1:1

- **The shell still poses the rider from the other machine.** `crates/openmx/src/main.rs` draws the
  body through `rider::Body::pose` (the 28-slot `game_sim::motion` frame), and `Chassis` still steps
  a `Stance` of its own against the bike's ten clips. So the pairing this task proves is proved *by
  test*, and the running shell is still the V9 wiring. Deliberate: V13's files are `vehicle.rs`,
  `anim.rs` and `tests/vehicle.rs`, and finishing the swap moves `rider.rs`, `main.rs` and the
  832-line `tests/body.rs` — three files other agents are working in this same hour. It is the next
  task, written below.
- **Which machine the *bike* should be stepped by is not settled by anything here.** The original's
  bike object is the other machine and its clips are that loader's `bike_*` names, which is strong;
  but no capture in this port has watched the bike's own animation block, so "the bike is driven by
  `game_sim::motion`" is inference from the clip names, not a measurement.
- **The `bike_` retry for the 36 pose slots** (ask again with the prefix when the bare name finds
  nothing) is still V9's reading of the loader, not a re-read of it. It is what binds
  `bike_air_Fwd/Mid/Bck` into states 35–37 on a bike, and `./kit fn 0x00258790` writes those three
  into named fields rather than through the table — so the port's path to the same place is not the
  original's path.
- **`start_phase` is still unmeasured** and still zero. V12 looked for it at `+0x8f8` and found that
  field is something else; the constructor leaves the five fade rates and the cap at zero too, and
  whatever writes them is not located.
- **`Chassis::want` is not a port.** The renderer still reads the wanted state off the ride
  (gesture slot, else `AIR`, else `TURN_RIDING`) instead of calling V11's ported chooser
  `vehicle_anim::Update`. All three tests here use that same reading, so what they prove about the
  pairing holds whatever the chooser would have said — but the state sequence a minute of the
  original would produce is not what they ran.
- **Only the lowest band is reached by riding.** The grid's upper regions are exercised by driving
  the request block (test 3), not by a ride that gets above 5 mph. The fixture is V9's drop-and-ride
  and it never does.
- **Nothing was looked at.** No screenshot, no emulator this session: the rider's pose is proved
  against the capture (through V12/V15's test) and against itself, never by eye. The one thing a
  human could add here is the obvious one — that the body on screen is in the pose the grid says.
- **The mix and phase axes are zero on the ride path.** `Chassis::request` writes both as zero
  because nothing in the port is known to write them; the original eases them towards something the
  body's own update computes. So a ridden frame sits at one end of every row instead of in the
  middle, and test 3's sweep is the only place the axes move.
- **`./kit check`'s total moved from 509 to 510** between runs of this session for a reason that is
  not this task's code: copying V12's capture into the worktree un-skips capture-backed tests. The
  verdict is the thing to quote, not the count.

## Next

Finish the swap the other way, as one task: pose `rider::Body` from the sixty-state machine (it is a
`Stance` now, with the rider's set and the `rider_`/`ff_rider_` prefix) and step the bike from
`game_sim::motion`'s 28-slot frame, then rewire `crates/openmx/src/main.rs` to hand each machine the
set it belongs to. The pieces are all here: `anim::Stance` takes any set, `anim::Rig` is the other
machine's rig, and V12's capture is the oracle for the rider half. The two things that task needs
and this one did not have are a writer for the request block's `mix_axis`/`phase_axis` — without it
a ridden body sits at one end of every grid row — and a human to look at the result once.

## At merge (V14 landed first)

Added by the merge, not by the session: V13 branched before **V14**, and one number above does not
reproduce on `main`.

V14 made `Set` resolve a clip name against every `.mcf` in its archives, first claim winning, because
V12's capture agrees with `airider.mcf` on three names and with the player's `mrider.mcf` on none.
Two of those three are `turn_stop_Fwd` and `turn_stop_Bck`, which the AI set aliases to
`turn_stop_Mid.MOT` — so **all three columns of the grid's stopped row are now one clip**.

`the_grounded_pose_grid_draws_across_every_band` measured "across a row" at a band of zero, where
`w3` is one and the grid plays that stopped row alone, so after the merge the fore/aft axis moved the
body 8.19e-7 there and the test's `across > 0.01` failed. The row is right and the measurement was
pointed at the wrong row: V14's own variant B — the one that left those two columns spelled out —
missed the capture by 3.6e-4 in this exact arm (state 30, band region 0, mode 4). So the assertion
moved to rows whose three columns are three clips, and the collapse is asserted rather than skipped:

    the furthest joint moves 1.607 across the standing-still row and 2.303 across the riding row,
    8.19e-7 across the stopped row (whose three columns are one clip), and 1.148 between the first
    band's row and the third's

The 1.485 quoted above was the *stopped* row's figure, measured when `mrider.mcf`'s spelled-out
Fwd and Bck columns were still the ones the port resolved. Everything else
in this entry — the pairing, the sixty slots, the minute, the measured `Params` — is unchanged, and
`./kit check` passes on the merge.
