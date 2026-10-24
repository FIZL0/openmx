# 1 — landing sound (FINAL)

Task P66. 2026-10-24.

## What I did

- **Found why the port's landing was loud and wrong.** P42 read `machine.res`'s `aud\machine.aud` and took
  the first section whose name ends in `Suspension`, which is `[buggySuspension]`. It played that on every
  bike landing: `Vehicle\suspen00..03` by speed tier and `Vehicle\trkbtm_2` (a truck bottoming out) from
  55 ft/s, at 50–100 %. The original's motocross bike plays no landing sound at all (below). A landing
  in the original is heard only as the engine note coming back from its airborne decel, which P49
  already ported.
- `crates/openmx/src/audio.rs`:
  - `Landing` is gone, and so is its play in `Sound::frame`. The trick-landing crowd cheer stays.
  - `Reading::machines` no longer reads `machine.aud` and returns only the plane's `LoopEngine`.
  - `SeatFrame::vy` and `SeatSound::last_vy` are gone too: nothing read them any more. `main.rs`
    lost the one line that set `vy`.
  - New `Mixer::clips()` lists the live voices' clips and gains. The test uses it.
- New test `crates/openmx/tests/audio.rs` `a_landing_plays_only_the_engine_as_the_originals_does`.
  It replays S109's small landing (12.9 ft/s down after 15 airborne ticks) and its big one (41.2 ft/s
  after 211) through `Sound::frame`, with the 125's engine, the fox rider and `machine.res` read off
  the disc. Every voice live on every tick must be one of the bike's engine samples, and none may be
  `machine.aud`'s suspension clips. I flipped the final assert once to prove the test runs and does not
  skip: it reported 216 engine voices heard over the small landing.
- Filed the other sounds this search turned up and left alone (not landings):
  - **P144**: the bike's `[Wreck]` clank. The original plays it on impacts while wrecked; the port
    plays one at the wreck entry.
  - **P145**: the drive-on surface loop.
  - **P146**: the biplane's tyre touchdown.
- opentennis has no vehicle or landing sound, so nothing was taken from it.

## What the original does

`research/oracle.txt` has the addresses, under "P66".

- **Who loads `machine.aud`.** Its sections are read by VehicleSoundEmitters.cpp's machine emitter
  (`./kit fn 0x003dcd10`, with `[<prefix>TireParams]`, `[<prefix>TireTouchdown]` and
  `[<prefix>Suspension]`). Only three classes make that emitter and call it: the car (`0x0038f5b0`),
  the buggy (`0x003990f0`) and the biplane (`0x00353a60`). `machine.aud` has Suspension sections for
  buggy, baja and monstertruck, and a TireTouchdown for the biplane. It has none for a motocross bike.
- **What the bike makes.** Its sound setup (`./kit fn 0x00280fb0`) makes four emitters:
  - the engine note, at bike+0x1494;
  - the `[Wreck]` clank emitter, at +0x1498 (`./kit fn 0x003dc570` loads it, and its frame is
    `./kit fn 0x003dc8c0`). It plays only while bike+0x948 is set, and that word is
    `(wreck reason copy +0x868) != 0`. It also needs the collision record touched and
    |bike+0x550| ≥ MinImpulse 18.8. So a landing that does not wreck never plays it;
  - the `Aud\event.aud` emitter, at +0x149c;
  - the drive-on emitter, at +0x14a0 (`./kit fn 0x001b9690` / `0x001b8da0`). It loops each wheel's
    touched *material's* `DriveOnSoundFile` from `Col\template.cmt`. On free09 that table lists only
    props (wood, metal, fences, hay, …, IDs 1–22), and every one of them has
    `DriveOnSoundFile=Impact\drv_drt1.vag`. There is no terrain entry.
- **The machines' rule, for when they are ported** (`./kit fn 0x003dd0a0`). The car calls it on the
  frame its collision record's "touched" word goes from 0 to set, with `v` = body+0xb0 rotated into
  the world:
  - s = |v.y|. Nothing plays when s is below tier 1's `VelocityTierStart`.
  - Otherwise it takes the last tier whose start ≤ s.
  - Its level is `MinLevelAttenuation + clamp01((s − MinLevelVelocity)/(MaxLevelVelocity −
    MinLevelVelocity)) × (Max − Min)`, as an int percent, with no ×1.46666 on these velocities.
  - Then `sound_3d` places it by distance.
  - There is no pitch curve, and no rate limit beyond the 0→touched edge.
- **The rider's `rider.aud`** has `[Fall]` and `[IMPACT]` only, and plays them on wreck reasons
  (S53). Neither is a landing.

## How it was proved

- **The code (primary).** I checked every emitter the bike constructs and every caller of the
  machine emitter's loader and play function. None is reached by a bike landing.
- **The original's own audio (supporting).** S109's `run2.mp4` (Video Capture of the animandpump
  playback) carries the SPU's sound.
  - Alignment: the video is one frame per game tick at 59.94 fps, so t ≈ (frame_counter − 1938)/60 s.
    I checked that against the HUD speedometer after the respawn: 4 mph at 33.7 s ↔ tick ~3959. It
    also puts the wreck at 3716 ↔ 29.6 s and the respawn at 3915 ↔ 33 s, both as seen in the frames.
  - Spectrogram (`showspectrumpic`, 2.5 s windows) at the big landings, 2751 ↔ 13.55 s and
    3459 ↔ 25.35 s: the low band changes there, which is the engine note switching. There is no
    separate thump.
  - Matched filter (scratch `xcorr.py`, normalised cross-correlation of each decoded clip against
    ±1 s of the recording):

    | landing | clips tried | real peak NCC | control at 0.2× | control at the port's gain |
    |---|---|---|---|---|
    | 2149 −12.9 | suspen00–03 | 0.06–0.08 | 0.23–0.31 | suspen00 0.65 |
    | 2149 −12.9 | trkbtm_2 | 0.09 | 0.61 | — |
    | 2185 −20.1 | suspen00–03 | 0.06–0.08 | 0.26–0.27 | suspen01 0.59 |
    | 2185 −20.1 | trkbtm_2 | 0.13 | 0.52 | — |
    | 2751 −41.2 | suspen00–03 | 0.06–0.08 | 0.08–0.15 | suspen03 0.35 |
    | 2751 −41.2 | trkbtm_2 | 0.13 | 0.25 | — |
    | 3393 −13.0 | suspen00–03 | 0.07–0.08 | 0.08–0.11 | suspen00 0.31 |
    | 3393 −13.0 | trkbtm_2 | 0.15 | 0.30 | — |
    | 3459 −38.5 | suspen00–03 | 0.06–0.08 | 0.12–0.14 | suspen03 0.43 |
    | 3459 −38.5 | trkbtm_2 | 0.14 | 0.34 | — |

    The control is the same clip mixed into the same window at that gain. It was found at +0.000 s
    every time; the real peaks fall at random offsets. Rerun at 2× clip speed in case the capture's
    audio is sped up: same result.
- **The port:** the new test above, over the small and the big landing.

## Not verified / not 1:1

- **No listening by ear.** This session cannot hear. The comparison is the code, the spectrogram and
  the matched filter above.
- **No side-by-side render of the port's ride against S109's audio.** The port's shell does not replay
  S109's ride tick for tick (S109's journal: the rides diverge within seconds). So the port side of
  the comparison is the test's per-voice check, not a spectrogram of the same ride.
- **The capture's audio level against the SPU's is unknown.** The matched-filter control assumes the
  mixer's gain 1 equals the clip's own level in the capture. At the loud landings (2751, 3393, 3459)
  a clip mixed in at 0.2× is barely above the noise floor. A very quiet thump there could have been
  missed by the filter alone; the code says there is none.
- **The audio-to-tick alignment is ±a few ticks.** It comes from the speedometer and the wreck frames,
  not measured exactly.
- **The test's landings are shaped, not captured.** It uses S109's journal values: the downward speed
  at touchdown and the airborne lengths, with a straight ramp of vy across the air. The capture's
  per-row vy was not fed, because no sound depends on vy now.
- **The machines' suspension and touchdown sounds are not ported.** The port has no drivable buggy or
  truck, and the biplane's touchdown is P146. Their rule is written in oracle.txt.
- **The drive-on surface loop (P145) and the wreck clank's impact rule (P144) are not ported.** Neither
  is a landing sound, and both are filed.
- **Not checked on any track but free09** (the template.cmt material list); the other tracks'
  `template.cmt` were only listed.

## Next

Done as briefed. Follow-ups P144, P145, P146.
