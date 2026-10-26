# 1 — plane crash sound (FINAL)

Task P164. 2026-10-26.

## What I did

- Found the original's plane crash sound: a wreck emitter the plane's audio setup builds from
  `machine.aud`'s `[biplaneWreck]`, the same emitter class as the bike's `[Wreck]` (P144). It is not the
  wreck/poof entry beside `[biplaneTireTouchdown]`, and not the event emitter the motocross plane adds,
  which is the shootoff's Cannonball (P85's), keyed on reason 13.
- Captured it in the original: `biplane.p2m2` played back in copy 3 with a watch spec on the plane's
  wreck word, collision record, shock peak and the emitter's own fields and five sounds (`capture/crash-sound.toml`),
  filmed with Video Capture at the same time (`capture/play_with_video.sh`). 1718 rows, frames
  10971..12688 with no gaps. Two crashes, four clanks. The data is in `context/capture/p164` (crash-sound.csv,
  crash.mp4) and is not committed.
- Ported it to `crates/openmx/src/audio.rs`: `MachineWreck` (the section, with the loader's defaults),
  `PlaneCrash` (what the emitter reads), `length`, `wreck_level`, `WreckClank` (one per seat, ticked in
  `Sound::frame`'s plane branch after the engine), `WRECK_TAIL`, `Reading::plane_wreck`, and new fields on
  `SeatFrame`, `SeatSound` and `Sound`. A pause hushes the clank, and a seat that leaves stops it.
- `crates/openmx/src/main.rs` `seat_sound`: each seat in the plane hands its `Flight`'s `crash.reason != 0`,
  `record.touching` and `|body.shock_peak|`. Every seat goes through it, local split seats and online ones alike.
- Tests in `crates/openmx/tests/audio.rs`:
  - `a_replayed_biplane_crash_clanks_as_the_original`: every captured tick from 11600 to 12700, over 32 seeds.
  - `a_plain_landing_plays_no_clank`.
  - `a_split_clanks_for_the_plane_crashed`.
  - `the_plane_crash_sounds_are_the_discs`.
  - In `crates/game-sim/tests/flight_collision.rs`: `a_plane_crash_is_heard`, the port's own dive into the ground.
- Follow-up S161: the port's plane crash hits its tires a tick before it wrecks (below).

## What the original does

The plane's audio setup (`./kit fn 0x00353a60`) builds a wreck emitter at plane +0x678
(`./kit fn 0x003dc490`). It loads it with `./kit fn 0x003dc570` and the prefix "biplane", so the section is
`[biplaneWreck]`:

| Key | Value |
| --- | --- |
| Streaming | TRUE |
| Looping | FALSE |
| Stereo | FALSE |
| Sounds | 5: `Vehicle\mchn_wreck1..5`, mono, 22050 Hz, 1.00 / 0.89 / 0.89 / 1.18 / 1.12 s |
| MinDistance | 30 |
| MaxDistance | 150 |
| MinImpulse | 5 |
| MaxImpulse | 20 |

The emitter is registered on the sound manager, not among the plane's children. `./kit fn 0x003dcb80`
points it at the body's shock peak: per axis, the largest velocity change the contacts gave the body this
frame, cleared in the preforce listener.

Each frame (`./kit fn 0x003dc8c0`, the bike's clank too), it plays a clank when all of these hold:

- the plane's wreck word is set;
- its collision record touched something this frame;
- none of the emitter's own sounds is still playing;
- `sqrt(z² + y² + x²)` of the peak is at least `MinImpulse`.

The clank is a no-repeat rand pick of the five. Its level is `clamp(int(|peak| / MaxImpulse * 100), 0, 100)`.
It is placed on the body and follows its position and velocity. "Still playing" is the streamed voice's
state on the sound chip, read through the stream manager. The sound object's handle stays set after the clip ends.

So the crash's own contact clanks on the wreck tick. While the plane stays down, the tumble's later hard
hits clank again once the last clank has ended. A landing is never wrecked, so it plays none, however hard.

## How it was proved

A PINE capture compared tick by tick, plus the capture's own audio:

- **Start tick and level.** The capture's sound objects set their level on 11664 (79, peak 15.97),
  11740 (72), 12498 (46, peak 9.23) and 12548 (100, peak 22.45). Each is `int(|peak| / 20 * 100)` of that
  tick's row, and 11664 and 12498 are the wreck ticks. The replay test feeds every tick's wreck word,
  touching flag and peak through `Sound::frame`, with the mixer run 1/30 s after each. It gets exactly
  those four ticks at exactly those gains, for all 32 seeds.
- **The quiet hits.** Ten gated hits over `MinImpulse` stayed quiet in the original (11665 11670 12499
  12500 12508 12509 12511 12521 12535 12555) and stay quiet in the port. So do the hits under 5 (12526).
- **Audible timing.** Matched against the video's audio (`capture/match_clips.py`), the clips sound at
  22.008 s (Sound1), 24.542 s (Sound5), 49.836 s (Sound3) and 51.502 s (Sound2). The gaps are 2.534,
  27.828 and 1.666 s; the ticks' gaps (76, 834 and 50 ticks) are 2.533, 27.8 and 1.667 s. So each clank is
  heard on the tick its level was set, and nothing else in the video matched a clip. The video's start is
  not stamped to a tick, so this proves the spacing, not an absolute latency.
- **The busy tail.** With `WRECK_TAIL` set to 0, the replay test clanks again at 12535 (level 33), which
  the original did not. With 0.4 it passes.
- **Plain landing.** The capture's landing at 12465 is not wrecked and its record is untouched, though
  its peak is 12.8; the original stays quiet. The port stays quiet too, even when the landing frame is
  marked touching, and also for a wreck with nothing touched and a wreck touched under 5.
- **Split and online.** Two seats each get their own clank; the seat flying on gets none, in either seat order.
- **The disc.** `[biplaneWreck]` reads 30 / 150 / 5 / 20, and its five clips are mono 22050 Hz with the
  lengths above, to 1 ms.

## Not verified / not 1:1

- **The busy tail is a constant, not the chip's voice state.** The original's "idle" is the SPU voice
  state of a streamed sound. That state trailed a 0.89 s clip by more than 0.34 s, and the voice was idle
  by 0.78 s past the clip's end. The port holds its emitter busy for 0.4 s after the mixer's voice ends.
  That is the largest tail that also fits the longest clip (1.18 s), whichever clip is picked, inside the
  one bound this capture gives. Why it trails (stream buffer drain, IOP polling) was not traced, so on
  other clips or timings it may be off by a tick or more.
- **The pick is the port's dice, not the game's rand().** The no-repeat rule is the original's; the
  sequence is not (the original picked Sound1, 5, 3, 2 here). The tests check the ticks and levels, not
  which clip.
- **Distance falloff.** `MinDistance` 30 and `MaxDistance` 150 go to the mixer's `near` and `far`. The
  mixer's curve was not compared with the original's 3D sound's. The original also hands the sound the
  body's velocity; the port has no doppler.
- **Level loudness.** The level is checked as the original's 0..100 value, and the port sets gain =
  level / 100. Whether the original's 46 is 0.46 of full loudness on its chip was not measured. The
  video's clanks are mixed with the engine and the camera distance.
- **The port's own crash clanks late.** The replay proves the clank given the original's inputs. In the
  port's own dive (45° into flat ground, `a_plane_crash_is_heard`), the tires take the hit a tick before
  the wreck. The wreck tick has the record touching but a zero peak, so the first clank comes 4 ticks
  later (peak 5.1, level 25). Whether the port's crash dynamics match the original's is S161.
- **Online not run live.** Online seats go through the same `seat_sound` and `Sound::frame` as split
  ones, but no networked ride was run.
- **The engine at a wreck.** I did not check what either side's engine loop does while the plane is
  wrecked; P164 left the engine alone.
- **Torn rows.** The capture's last-pick field read one row late at 11664 and 12498. The level was read
  off the sound object's own field, which was on time.

## Next

Nothing for P164. S161 for the port's crash dynamics.
