# 1 — wreck sfx (FINAL)

Task P180. 2026-10-26.

## What I did

- **I looked for the regression and found none offline.** At c67d9b2f a wreck ride plays its voice and its clank on every path I could drive unattended:
  - headless `--play` of a generated tape (gas, then the left stick pulled back from tick 240, which wrecks with reason 11 at about 10 s; a stick pushed forward gives reason 8);
  - windowed `--play --vsync off`;
  - `--garage --tricks mxvatv`, the user's own settings (tricks=mxvatv, effects_volume=80).

  To hear what actually came out, I swapped in a fake `pw-cat` that writes the stream it is given to a file. That stream's RMS jumps from about 2,900 to 8,200–8,800 at the wreck's sample (10.0 s). The voice and the clank start at gain 0.8 per ear, with the listener 9 ft from the bike. The clips decode to real sound (pre_fall rms 1992, WRECK rms 4761). There was nothing to bisect.
- **I found one path that really is silent: online rides.** The online tick sets `world.seats[0].said` to `rider::Frame::default()` every tick (main.rs, beside `world.seats[0].hands = on.hands`). So `Sound::frame` never sees a wreck entry there, and an online wreck has no voice, no clank and no trick cheer. Filed as P184. It is not fixed here, because the confirmed-versus-predicted ride is a rollback question of its own.
- **What the user most likely means by "the sounds are gone": the port's fall wreck is a 0.3 s grunt followed by a silent tumble.** I ran a matched filter (normalised cross-correlation, the P66 method) of all 14 wreck clips over S109 run2's own audio, for its reason-11 wreck at tick 3716:

  | clip | list | time in run2.mp4 | tick (≈) | peak NCC | next best |
  |---|---|---|---|---|---|
  | Rider\pre_fall3 | [Fall] | 29.66 s | 3716, the wreck's entry | 0.517 | 0.087 |
  | Vehicle\WRECK05 | bike [Wreck] | 30.82 s | 3787, a_touching rises | 0.507 | (WRECK04 0.29, same place) |
  | Rider\FALL02 | [IMPACT] | 30.93 s | 3794 | 0.568 | — |
  | anything else | | 26–29.4 s and 31.9–34.5 s | | below noise | |

  So the original's fall is a grunt at the entry and then a scream when the body hits the ground, about 1.2 s later. The port had the grunt only. Its clank comes at the entry instead of at the bike's touchdown, which P144 already covers.
- **I ported the original's rider voice past the entry** (audio.rs `RiderVoice`, `RiderBody`; `RiderSounds::min_impulse` read from rider.aud [Params] MinImpulse = 12.8). It runs every tick, after the simulation. When the body is down for a reason other than 0 or 13, touching, and faster than MinImpulse, and the voice has nothing of its own still playing (the entry's cry counts, plus the streamed tail `WRECK_TAIL`), it plays a no-repeat pick of [IMPACT]. The pick shares the entry's [IMPACT] "last", plays at full level on the body, and follows it while it plays. The shell feeds it `RiderBody::of(&rider)`: the ragdoll's own sensors (`Down::sensed`) when the ride runs one, else the wreck's one rider point (`Crash::landed`, `rider_vel`).
- **Tests** (crates/openmx/tests/audio.rs):
  - `a_wreck_ride_hears_the_voice_and_clank` runs game_sim's drop-and-wreck fixture at 30 Hz (game-sim's tests/ride.rs bike and wreck files). Reason 5 plays an [IMPACT] cry and the [Wreck] clank on the wreck's own tick. After it, only [IMPACT] screams play, on touching ticks, and there is at least one when the rider comes down.
  - `a_wreck_ride_hears_the_discs_voice_and_clank` is the same ride on the disc's rider.aud and the 125's [Wreck]: 5, 5 and 4 clips, near 30, far 150, MinImpulse 12.8. Its 0.9 s entry scream still holds the voice when this rider lands, so no touchdown scream is required.
  - `the_rider_cries_only_down_touching_and_fast` covers the gates and checks that the grunt plus its tail hold the scream off.
- **The other wreck effects, against S109.** I compared two sheets of frames at 0.2 s apart: run2.mp4 from 29.3 s, and the port's `--headless --shot` frames 290–440 of the wff tape on its default track. They are different wrecks on different tracks, so only qualitative differences count.
  - **HUD:** the original's speed readout drops to 0 at the wreck and stays there until the put-back. The port shows the wreck bike's speed (33 falling to 17). Filed as P182.
  - **Rumble:** the player's wreck entry rumbles a human's pad at (1.0, 200 ms) unless vibration is off. The shell has no rumble at all. Filed as P183.
  - **Dust and debris:** both throw dark clods where the bike hits (P156). I saw no difference worth a task at this resolution.
  - **Camera:** the original's wreck camera drops low and close behind the thrown rider, and the bike leaves the frame. The port's stays further back and higher. The two wrecks are not the same, and V31 already ported the wreck camera from the decompilation, so I did not file this as a gap (see Not verified).
  - **Sim RNG:** the touchdown cry's pick and the bike clank's picks both draw rand(), and game-sim does not model them. Filed as S173.
  - **P144:** the wreck entry's own clank is gated on MinImpulse too. Added as a note on P144.

## What the original does

- The rider emitter's frame is `./kit fn 0x0033e790`:
  - It acts only when +0x23c == 0.
  - It reads the body record at +0x240: reason +0x104 must be neither 0 nor 13, +0x1bc must be set, and |+0x130..+0x138| must exceed MinImpulse (+0x238).
  - The emitter must be idle (vt +0x90 == 0).
  - It then calls `0x0033e850(emitter, 1)`.
- `./kit fn 0x0033e850`: 0 picks [Fall] (+0x200/+0x208/+0x218) and 1 picks [IMPACT] (+0x21c/+0x224/+0x234). The no-repeat pick `0x001b5820` draws rand(), and `0x0033e900` plays the result with no level set.
- The player's wreck entry (`./kit fn 0x0027b810`, player_wreck_entry):
  - It calls `0x003dcb90(bike+0x1498, 0)`, which runs `0x003dc730` when the bike's [Wreck] emitter is idle. That plays a clank only when |*+0x224| ≥ MinImpulse, at min(100, |imp|/MaxImpulse·100)%, with a rand() no-repeat pick.
  - For a human player it rumbles the pad at (1.0f, 0, 200) unless the options' word at +4 is set.
  - It tells two more objects (`0x003b3940` with 2, or 6 for reason 7, and 5; and the network session when there is one), and the screen (`0x001ffcb0`, screen_wreck_notice).
- All of this is now in research/oracle.txt under P180.

## How it was proved

- **Original:** a matched filter of the disc's own clips over S109 run2.mp4's audio (22050 Hz mono, from ffmpeg), compared with run2.csv's rows (wreck_reason, a_touching, a_off_bike).
- **Port:**
  - The rendered output stream of three kinds of local ride (above).
  - Three tests: one on a real game_sim wreck ride with tone clips, the same ride on the disc's clips, and one on the gates.
  - Frame sheets beside each other for the effects.
- The decompilation was read with `./kit fn` only.

## Not verified / not 1:1

- **No listening by ear.** Everything here was judged by RMS and by the matched filter. Nobody heard the shell's output.
- **The user's own path stays unreproduced.** The front end's ride (menus, then Ride) cannot be driven unattended here: the front end ignores `--pad`, and `--menu --screen pause --play` stays frozen under the pause. A front-end-only cause of the silence would not have shown up, and none was found in the code.
- **Field meanings.** The +0x1bc and +0x130 fields of the emitter's body record are read as "touching" and "velocity" from their use, not proven. In S109, a_touching (a different read path) rises at tick 3787 and the scream lands at about 3794, 0.1 s later. The matched filter's alignment is ±1–2 ticks.
- **The speed held against MinImpulse is the port's choice: the larger of this tick's body speed and the last tick's.** The no-ragdoll rider point stops dead on the tick it lands, which would otherwise mute every touchdown. With the ragdoll (the shell's default when the disc has the kit), the main bone's velocity is used as-is. The original's body velocity on a touching tick was not captured.
- **The voice's busy tail reuses the bike clank's `WRECK_TAIL` (0.4 s).** [IMPACT] is streamed too, but the rider voice's own tail was not measured.
- **The touchdown pick does not draw the game's RNG in the sim.** Only presentation dice draw for it (S173). The same holds for the bike clank picks.
- **The camera comparison is inconclusive.** The two filmed wrecks differ in track, speed and reason. V31's port of the wreck camera was not re-checked against S109's rows; run2.csv has no camera columns.
- **The wreck entry's other two notices** (`0x003b3940` with 2/6 and 5, and the network session's call) were not identified. Nothing audible or visible came of them in S109's freeride wreck.
- **The bike clank still plays at the entry, unconditionally and at full level.** That is left to P144, which now has the entry gate noted on it.

## Next

Nothing for P180. Follow-ups: S173 (sim draws), P182 (HUD speed 0), P183 (rumble), P184 (online wreck sounds), P144 (clank gating).
