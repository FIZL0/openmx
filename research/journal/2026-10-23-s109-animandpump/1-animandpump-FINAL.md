# 1 — animandpump (FINAL)

Task S109. 2026-10-23.

## What I did

- Played the user's `animandpump.p2m2` back on the original from its own save state (PCSX2 copy 2,
  the pause-before-Play recipe, `capture/play.sh` + `capture/animandpump.toml`) and captured it twice.
  `run1` covers frame_counter 1939–3142, 1204 rows of riding with no wreck, and is the clean one for
  tick-by-tick work. `run2` is the whole movie: 2361 rows over 1939–~4330, with a wreck. 265 of its
  rows are not one step from the last (torn or skipped reads). Each run was filmed with Video Capture
  (`run1.mp4`, `run2.mp4`), and the state's RAM and scratchpad were dumped (`start.ee.bin`,
  `start.spr.bin`). All of it is in `context/capture/s109/`, git-ignored.
- **The pump meter (V53)**: `crates/openmx/tests/overlay.rs`
  `the_preload_meter_is_the_originals_over_animandpump` steps the port's `PreloadMeter` from each
  row's sense against the original's meter word. Over run2: 2361 rows, 330 lit in 8 runs, 0 bad. The
  hide is set on the 187 wreck rows. Nothing needed fixing.
- **The rider's body on the bike.** The original draws a riding rider from the **riding ragdoll**,
  which the body actor steps every tick (S70's oracle note: "While riding, the skeleton IS the riding
  ragdoll"). The lean across a slope and the body's give on the bike come from that step, not from
  the clip. The port already had the actor (`ragdoll_body::Actor`, S61), but the shell ran it only
  in a wreck.
  - `crates/game-sim/tests/ragdoll_riding.rs` proves the port's riding tick bit for bit against the
    capture: 1203 of 1203 ticks.
  - New `crates/game-sim/src/ragdoll_riding.rs` holds `Riding` (the actor with the original
    constructor's numbers), `request` (what the bike hands it, built from the ride) and `Riding::step`
    (one tick, with the skeleton's points and rests taken from the animated pose).
  - `crates/openmx/src/main.rs` steps a `Riding` per seat right after the chassis frame, with the hands
    on the chassis' live grips and the feet on its pegs. The body on the bike is drawn from the
    ragdoll's nodes (`ragdoll_palette` / `skin_ragdoll_into`, the path a wreck already used) once the
    ragdoll is placed.
  - It is draw-only. Nothing in the ride reads it, it has its own generator, and it is not run online.
    The `ride hash` of a `--play animandpump.p2m2` run is the same before and after the change.
- **Frame ranges** for the tasks that will use this capture are below.

## What the original does

- The rider body actor is at `0x015b96f0` in the state (the human's vehicle). Its update is
  `rider_body_update` (`./kit fn 0x0024e640`). The riding request it reads is at `0x151c990` (actor
  `+0x488`).
- `rider_physics_update` (`./kit fn 0x0030f6e0`) runs these steps in order:
  1. `rider_part_sync` (`./kit fn 0x001b2640`) for each part, which is the skeleton point each bone's
     pose spring pulls to (its centre of mass through the animated node);
  2. vt+0xa8 (the animation);
  3. the world step;
  4. vt+0xac.

  After the step, `0x0030f4c0` normalises each body's quat and writes its node back as
  `body.pos − turn(q, cofg)`. That node is what is drawn.
- The constructor's numbers, read off the state:

  | Field | Value | Note |
  |---|---|---|
  | stand speed `+0xa14` | 7.3333 | `0x40eaaa65`, the clutch's idle speed |
  | slow speed `+0xa20` | 73.333 | `0x4292aa7f` |
  | foot offsets `+0x510`/`+0x520` | (−0.02028, −0.10772, −0.16558) and (0.03224, −0.10772, −0.16558) | |
  | pose spring `+0x650` | (80, 30, 30) | `[Joint Forces]` |
  | damper base `+0x670` | (3.2, 2, 2) | `[Joint Forces]` |
  | active / torques / gravity | 1 / 1 / 1.0 | |

  The request's bike box is at `+0xf0`/`+0xf4`: centre (0, 4.3, −1.5), half-size (0.45, 0.6, 1.75).
  The bike collider at `+0x1ac` has local centre (−2e−5, 3.0166, −0.0312) and radius 4.3035.
  `the_riding_bodys_numbers_are_the_originals` checks the port's `Riding::new` against all of these.
- What the request holds while riding:

  | Field | What it holds |
  |---|---|
  | rows | the chassis quat's basis |
  | frame | the drawn transform (ahead of the rows by the no-hander roll, P32) |
  | centre | ≈ the chassis' world centre of gravity |
  | velocity | the chassis velocity |
  | rates | ≈ the chassis angular velocity in its own frame (`z` is the spin the pose is carried round by) |
  | hands, feet | world points at the grips and the pegs |
  | pitch | ≈ the body machine's mix lag (`shown_pose` in the capture) |
  | speed | not \|velocity\|; see Not verified |

## How it was proved

- **The riding tick, bit-exact**, in `crates/game-sim/tests/ragdoll_riding.rs`
  `the_riding_tick_is_the_originals_from_each_row`. For each run1 row:
  1. lay the original's bones in from the row before;
  2. run the original's `rider_body_update` in the EE with its skeleton points, rests and scene
     answers hooked and recorded;
  3. run the port's `Actor::tick` from the same actor and request read off RAM, with those answers.

  Every number the update writes matches on **1203 of 1203 riding ticks**.
- The seeded original against its own capture (exploration, `the_originals_tick_from_each_row`)
  matches on 1202 of 1203. The miss, at 1940, is a capture quirk two steps after the state.
- **The constructor's numbers**: `the_riding_bodys_numbers_are_the_originals` checks the kit built
  from the disc's `mrider.phy` against the state's actor, bit for bit.
- **The pump meter**: the overlay test above, over 2361 captured ticks.
- **The look, by eye**: `openmx --track free09 --trace-at 4309.7,3317.2,0 --play animandpump.p2m2
  --headless --shot … --shot-frame N` at N = 60, 200, 300, 500, 700, with this branch and with HEAD
  built in a scratch worktree. The crops are in `context/shots/s109/` (`head-vs-port.jpg`,
  `orig-head-port-slope.jpg`, git-ignored).
  - The rider stays on the bars and the pegs at every frame.
  - On the slope, the port's torso stays more upright against the leaning bike than HEAD's clip pose
    does. The original's rider (run1.mp4 at 4–5 s) does the same.
  - The ride is identical in both builds (same end positions and hash).

## Frame ranges (run2, frame_counter; one row a tick)

The movie has 2408 frames for about 2410 captured ticks, so one movie frame is one tick and the
movie frame is about frame_counter − 1938. run2.mp4 is only 44 s long, so it shows about the first
1320 ticks (to about 3260).

- **Landings (S67 how the bike lands, P66 the landing sound)**: the tick the airborne flag drops,
  with the vertical speed just before:

  | Tick | Vertical speed (ft/s) |
  |---|---|
  | 2149 | −12.9 |
  | 2185 | −20.1 |
  | 2187 | −20.7 |
  | 2751 | −41.2 (the big one) |
  | 2996 | −5.2 |
  | 3330 | 0.4 |
  | 3372 | −5.6 |
  | 3393 | −13.0 |
  | 3395 | −13.4 |
  | 3459 | −38.5 |
  | 3499 | −5.1 |
  | 3923 | −4.5 (the wreck coming down) |
  | 4071 | 0.2 |
  | 4076 | −0.5 |
  | 4104 | −3.7 |
  | 4111 | −6.1 |
  | 4116 | −8.0 |
  | 4123 | −9.9 |

  Airborne spans: 2134–2148, 2183–2184, 2186, 2540–2750, 2948–2995, 3319–3329, 3369–3371,
  3382–3392, 3394, 3401–3458, 3492–3498, 3604–3922, 4068–4070, 4075, 4101–4103, 4108–4110, 4115,
  4120–4122.
- **Particles (P74)**: the dust and roost follow the wheels' contact. The grounded stretches lie
  between the spans above. The hard landings at 2751 and 3459 are the ones to look at for landing
  dust. The stop near 3151–3213 is where the bike idles.
- **The wreck**:
  - wreck_reason 11 from 3716;
  - the body actor's mode is the wreck's (2) from 3718, flung at 3722;
  - off the bike at 3778, touching at 3786;
  - reason 0 at 3913, riding again at 3915.

  The mode is not riding at 2996–2997, 3094–3095, 3156–3157 and 3718–3914.
- **The pump (preload ready)**: 2253–2296, 2343–2356, 2510–2540, 2918–2948, 3039–3093, 3310–3320,
  3570–3604, 4221–4331.
- **The lean across a slope**: |lean_x| > 0.5 at 2007–2078, 2128–2142, 2188–2241 (broken),
  2275–2309, 2371–2431 (broken), 2600–2661, 2771–2839, 2867–2900, 2927–2945, 3181–3274,
  3360–3472, 3973–4061. |lean_y| > 0.5 at 2526–2544, 2562–2602, 2635–2648, 2754–2775, 2859–2897,
  3288–3366, 3668–3724, 3922–3994, 4032–4058. 2284–2434 is a long sideways traverse (roll down to
  −0.95).
- **V35 the flip pose**: **nothing.** No trick runs and no flip is on anywhere in the movie.
- **The bike's own animations and the engine (P42/P74)**: the whole ride. It stands still at the
  start (1939–~2010), climbs the slope (2010–2130), idles near 3151–3213, and tops out at about
  60 mph.

## Not verified / not 1:1

- **The request the shell builds is not proved.** The tick is bit-exact only from the original's own
  request read off RAM, not from what `ragdoll_riding::request` builds out of the port's ride.
  - **speed**: the request's speed is not |velocity| (49.7 against 69.07 at one row, 102.5 against
    102.0 at another). The port uses the chassis' forward speed (`local_vel.z`) as the nearest
    guess, but its source in the original was not found.
  - **pitch**: the port uses the vehicle animation's mix lag. It is close to the capture's
    `shown_pose` but was not checked to the bit.
  - **rates**: the port uses the chassis' own-frame angular velocity. It is close but not exact.
  - **hands and feet**: the port uses the chassis' live grip and peg dummies through the ride's
    transform, not the original's hand dummies (`Dummy_Hand_*`, `Dummy_foot_*`), whose world points
    were only read at the state.

  So the shell's riding ragdoll is the original's step from a nearly-right request, and its bones
  are matched by eye, not to the bit.
- **The skeleton's points and rests come from the port's posed rig**, through `ragdoll_pose::pose`
  (the node's basis turned onto the cofg). The original's `rider_part_sync` builds them from the
  skeleton's own matrices, and that build was not compared.
- **The riding scene is open.** The riding bones are tested against the ground (the shell's `Place`)
  but not against the scene's meshes. In the original's riding ticks the scene answers were only
  replayed, and their source was not ported for riding.
- **The look-around** (looking, tucked, the feet switch) runs from the request's defaults. Only
  `clip` (the vehicle animation's state) and `pitch` are fed. `looking`, `tucked`, `posed` and `cut`
  are never set, so the start line's gate and the look-around poses are not driven.
- **Online seats do not run it.** Online, the session poses the body inside its ticks. A riding
  ragdoll there would have to roll back, so it is left out and online riders keep the clip pose.
- **The wreck entry is still taken from the animated skeleton**, not from the riding ragdoll's
  bodies. In the original, a wreck starts from the bones the riding step left. Filed as a follow-up.
- **The lean is by eye**, from a teleported start (`--trace-at`) that does not reproduce the
  original's ride tick for tick. The two rides diverge within seconds, so no shot shows the same
  tick in both games.
- **The video is not aligned to the ticks.** run2.mp4 covers about 44 s from the movie's start, and
  the frame↔tick offset was not measured.
- **The airborne spans are the capture's airborne flag.** The long one, 2540–2750, was not checked
  against the wheels' contact. Its wheels read no hit throughout.
- **No flip and no trick in the recording**, so V35 gets no range from this capture.

## Next

Done for S109 as briefed, apart from the items above. Follow-ups filed: S110 take the
wreck entry from the riding ragdoll's bodies (and the checksum), S111 the request's sources, P130 online,
C8 a recording with a flip for V35.
