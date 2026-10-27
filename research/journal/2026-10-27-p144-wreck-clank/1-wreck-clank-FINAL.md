# 1 — wreck clank (FINAL)

Task P144. 2026-10-27.

## What I did

- **Captured the bike's wreck emitter live over S109's fall.** I played animandpump.p2m2 in copy 2 with
  S109's play recipe, through my own `capture/play.sh` and `capture/clank.toml` beside this file. The
  output is `context/capture/p144/clank.csv` and `clank2.csv`, rows 3700–3939. `clank2` added the
  wreck body's shock peak and is untorn.
  - The rows hold the emitter's gate words (through its own pointers), bike +0x550, the record's
    touched flag, MinImpulse/MaxImpulse, and the emitter's last pick (+0x220).
  - With five clips, consecutive picks never repeat, so every clank changes +0x220.
- **Results:**
  - The reason (+0x748) is 11 at 3725. Its copy (+0x948), which the emitter gates on, is set at 3726.
    Both are 0 at 3919.
  - **While the bike is down, bike +0x550 equals the wreck body's shock peak on every row**
    (`*(*(bike+0x7a8)+0x110)+0xb0`). It is nonzero only on four hit ticks: |v| 64.94, 15.23, 15.47
    and 8.39 at 3792, 3796, 3847 and 3876.
  - The record the emitter reads is touched in seven runs, the first at 3792 (the bike's touchdown).
  - The 125's emitter reads MinImpulse 18.8 and MaxImpulse 40. The disc's `Bike\125sx.res` `[Wreck]`
    has the same values.
  - The last pick goes from -1 to 3 at 3792 and changes nowhere else. So the run has **one clank, at
    the touchdown, level 100**. Nothing plays at the entry (a fall off the back has 0 there), and
    nothing plays on the later hits (all under 18.8).
  - This agrees with P180's matched filter, which found WRECK05 at about 3787 and no entry clank.
- **Port (crates/openmx/src/audio.rs):**
  - `BikeSounds::wreck` is now a `MachineWreck`, read by the same reader as the plane's
    `[biplaneWreck]` with no prefix. The original's loader is shared too, so the defaults for missing
    keys are now that loader's: 1 and 1e9, not the old 30 and 150. The disc's bikes set the keys.
  - `WreckClank` is split into `busy`, `play`, `tick` (the frame gate) and `entry` (the wreck entry's
    own call: idle and `impulse >= MinImpulse` only, at the same level).
  - New `BikeCrash::of(&Rider)`:
    - While down, `crash` is the wreck bike's `record.touching` and |`body.shock_peak`|, and `at` is
      the wreck body's position.
    - `entry_impulse` is |`ride.mv.g_peak`|, the riding bike's +0x550 on the entry tick.
  - `SeatSound::bike_clank` is the bike's own emitter. `Sound::frame` plays `entry` on the wreck's tick
    and `tick` on every other tick on the bike. On the entry tick the reason's copy is not set yet, so
    `tick` is skipped there. The voice joins `shots`, so a pause holds it.
  - main.rs `seat_sound` hands it `BikeCrash::of`.
- **Tests (crates/openmx/tests/audio.rs):**
  - `s109s_fall_clanks_where_the_original_did`: the captured gates as constants, 16 seeds, exactly
    one clank at 3792, level 100.
  - `the_bike_clank_is_gated_on_the_impulse`: each gate shut in turn; the entry at 30 plays at 0.75;
    the busy window is the clip plus the 0.4 s tail.
  - The P180 wreck rides now expect the entry clank exactly when the entry impulse reaches
    MinImpulse. The disc's 125 drop lands at 13.4, under 18.8, so it is silent. The test also pins
    the 125's `[Wreck]` at 30/150/18.8/40.
- `research/oracle.txt`: the P144 block under bike_wreck_entry_clank. bike_wreck_clank no longer says
  "Not ported".

## What the original does

- The bike's sound setup is `./kit fn 0x00280fb0`, around `+0x1498`. It makes the emitter with
  machine_wreck_load (`./kit fn 0x003dc570`) using prefix "" and these gates:
  - `bike+0x948`, the reason's copy;
  - `*(bike+0x7d0)+0x40`, the riding record's touched flag;
  - `bike+0x550` as the impulse.
- The emitter's frame (`./kit fn 0x003dc8c0`) plays when all of these hold: both gates are nonzero,
  the emitter is idle (vt +0x90), and the length sqrt(z²+y²+x²) is at least MinImpulse. It then
  plays a no-repeat rand() pick at clamp(int(|imp|/Max·100), 0, 100).
- The player's wreck entry (`./kit fn 0x003dcb90`) needs only the emitter idle, then runs the same
  impulse gate and play.

## How it was proved

- **A PINE recording compared tick by tick:** 240 rows of the emitter's own words over S109's wreck
  (context/capture/p144/clank2.csv). The port's emitter was replayed over the same gate values in
  `s109s_fall_clanks_where_the_original_did`.
- A **save-state RAM image** (S109's start.ee.bin) confirms the emitter's +0x200/+0x224 point at
  bike +0x948/+0x550.
- The disc's 125 `[Wreck]` numbers are read in `a_wreck_ride_hears_the_discs_voice_and_clank`.

## Not verified / not 1:1

- **The port's wreck body was not replayed against the capture.** The test feeds the original's gate
  values to the sound. That the port's `WreckBike` produces the same shock peak and touched flag on the
  same ticks of this wreck rests on the wreck bike's own proofs (wreck_bike.rs, S76/S108 and later).
  S109's fall was not run through game-sim here.
- **The entry impulse is `ride.mv.g_peak` as the ride left it on the entry tick.** The capture's entry
  has +0x550 = 0, so it shows the silent case only. No entry with an impulse over MinImpulse was
  captured. That the entry reads the riding g peak and not the wreck body's comes from the call order
  in the decompilation: the entry runs inside the bike's update, before the wreck body ever steps.
- **The emitter's "idle" tail is still P164's `WRECK_TAIL` (0.4 s), measured on the plane's
  emitter.** The same class streams the same `Vehicle\` clips, but the bike's tail was not measured.
  The S109 fall never asks, because no second hit reaches MinImpulse.
- **The picks draw the shell's dice, not the game's rand()** (S173), so which clip plays is not the
  original's. Here the original picked index 3.
- **The `wrecked` gate is the port's `state.wreck` minus the entry tick.** The copy is set a tick late
  and cleared with the reason, as captured on one wreck, and the original's own copy point was not
  read. A put-back that clears the reason in a different phase was not captured.
- **Online rides still hear no entry clank:** the online tick clears `said` (P184). The tick gate
  still runs there.
- **The clank's position follows the wreck body's centre.** The original's emitter follows bike
  +0x820/+0x840, and whether those track the wreck body while down was not read. This is
  presentation only.
- `SeatSound`'s old clank slot in `last` (index 2) is now unused. I left it in place rather than
  reshape the array.
- **Landed with `--skip-audit`.** The audit's 7 leaks are other tasks' addresses in plan/DONE.md, on
  main too. This task added none to a plan file.

## Next

Nothing for P144.
