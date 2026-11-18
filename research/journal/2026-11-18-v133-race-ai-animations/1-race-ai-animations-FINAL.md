# 1 — race ai animations (FINAL)

Task V133. 2026-11-18.

## What I did

- **Checked first what the bots already do.** V128 already drives each AI rider's body through the
  human's path: `Body::ride` off its own rider (the 60-state body machine), the riding ragdoll
  (S90/S110) held to its own chassis, its own wreck ragdoll. A temporary per-tick trace (removed) of
  sx01, `--race --racers 4 --race-bot`, 600 frames showed for every AI rider:
  - gate states 53 → 46 → 58 at the start, then the riding grid (30) and the air row (35) by turns,
    the grid's two-clip blend moving with the brain's bar, and the lean axis at 0.75 / −0.3 / −1 in
    the air;
  - a wreck (player 4: 40, 43, Wrecked, then Held and back to Riding), thrown by its own ragdoll;
  - its ragdoll kit, wreck files, holds and riding ragdoll all present, placed while riding.
- **Checked the original does the same.** In the S282c sx01 race image (`context/states/
  s282c-sx01-race.ee.bin`), all four riders' body actors (found by +0x230 = 22 joints and +0x2e0 = 18
  parts; see oracle.txt S160's actor layout) have the riding ragdoll on: active +0x2b0 = 1, repose
  +0x2a8 = 0, placed +0x2ac = 1 on the three in state 30 and 0 on the one in the air (state 35).
  So the original's AI riders are drawn from the riding ragdoll like the human. The task's guess,
  "not S90's ragdoll spring, which is the human's", is wrong: drawing the bots from clips alone
  would make them less like the original, so I did not do it.
- **Measured how tall the bodies stand.** Head-to-feet span of the 18 ragdoll nodes in world y:
  - the original (image above): 3.86, 3.24, 3.81, 4.42 ft; pelvis above the feet 1.85–2.69 ft;
  - ours, all 4 riders over ~230 riding (state 30) ticks each: mean 3.67–3.72 ft, range 2.84–4.59 ft;
    pelvis 2.20–2.25 ft on average. Same range; the bots and the race-bot human agree with each other.
- **Fixed the one wrong thing found: the `--race-bot` human's body.** Its body machine read the pad
  (`seat.hands`, all zero), so the brain-ridden human sat in gate state 58 for the whole race, drawn
  upright with its arms out while riding at 55 mph (seen in a shot). It now reads the controls its
  brain hands the bike (`pilot_hands`, shared with `ai_ride`): after the fix it leaves the gate into
  state 30 on the drop tick like the bots. A normal human seat has no brain, so `pilot_hands` is
  `None` and it still reads the pad.
- Shots beside the original: PCSX2 copy 1 on `c31-sx01-race-start.p2s`, frames saved by
  `./kit pine save 9` at frame counter 60..240 and read out of each .p2s's Screenshot.png; ours by
  `openmx --headless --track sx01 --race --racers 4 [--race-bot] --shot-frame N`. At the drop the
  original's bots crouch in the attack stance, one standing, one sat; ours ride in the same stance.
  `OPENMX_RIDING_RAGDOLL=0` (clips alone) draws our bots taller and stiffer than the original's.
- Took nothing from opentennis: it has no AI riders on bikes.

## What the original does

Every rider in a race, human or AI, has a body actor of the same class. Its update runs the body
machine and then the riding ragdoll while placed and active (`./kit fn 0x0030f6e0`,
rider_physics_update in oracle.txt). So the AI riders get the same clips and the same spring.

## How it was proved

- Save-state RAM: the s282c sx01 race image's four body actors (mode, state, placed/active/repose,
  node heights), read with a scratch Python script.
- Looked right beside the original's race-start frames (sx01, the c31 start state), by eye.
- The trace numbers above (states, blends, ragdoll spans) from our own headless race.

## Not verified / not 1:1

- **No frame-by-frame match of a bot's pose with the original's.** The original's race start and ours
  are not the same race (the c31 state's human is already riding, our human sits at the gate or
  rides on our brain), so the bots are in different places. Compared by states and body heights only.
- **The original's race RAM image is one moment.** The placed/active words and the heights are four
  riders at one tick, not a run.
- **Every bot still wears the human's model (`mrider`, not the original's low-detail `airider`), gear
  and paint.** Outfits are the separate "random outfits" task; the `airider` body is not loaded.
- **The bots' clip set takes the player's `.mcf` claims (`Set::alone`).** With AI riders on the track
  the original's set takes `airider.mcf`'s three grid aliases first (V14/V121): `turn_stop_Fwd/Bck`
  play `turn_stop_Mid`, `turn_still_Bck` its own clip. Only shows when a rider is stopped or crawling; filed as V136;
  not changed, because it changes the human's set too and V121 decided it for the lone rider.
- **No test for the `--race-bot` body fix.** Checked by the removed trace (state 58 → 30 at the drop)
  and a shot. `--race-bot` is a test flag; a test would have to run a whole headless race.
- **Why the user saw the bots as wrong is not pinned down.** I found no AI-only difference in the
  animation path. Possible causes left: the same outfit on every bot (other task), the bots' riding
  line, or something seen in a build before V128 drew them at all.

## Next

none
