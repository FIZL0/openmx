# 1 — bike ride sounds (FINAL)

Task P285. 2026-11-03.

## What I did

- Dumped every `.aud`, `.cmt` and `.pb` on the disc (517 files) and listed the sound bank
  (`Aud\audio.res`, 279 files), using throwaway `game_render::disc` examples that wrote to the
  scratchpad. Nothing was kept, and nothing went under `crates/`.
- Read the four bike files (`vehicle_125a/250a/450a/50a`; 250a has 16 identical copies), `rider.aud`,
  `machine.aud`, `event.aud`, all 54 track `.aud` files, and `Col\template.cmt`.
- Decompiled the code that plays each non-engine sound: the crowd's player and its callers, and the
  material table's collision listener and per-frame player.
- Read what the port plays: `crates/openmx/src/audio.rs` `BikeSounds`, `Note`, `RiderSounds`, `DriveOn`,
  `WorldSounds`, `Sound::frame`.
- Searched S109's `run2.mp4` for each candidate clip with a matched filter. The method is below.
- Wrote the table to `research/bike-sounds.md`, the addresses to the P285 block in `research/oracle.txt`,
  and filed the follow-ups.

## What the original does

**The bank.** No sample exists for a gear shift, backfire, rattle, skid, roost, splash, or a surface by
ground. The bike's sounds are the engine sets ([Idle] [LowAccel] [HighAccel] [Power] [Sputter] [Decel]
[Clutch] [Panic]), [Wreck], the drive-on loop `Impact\drv_drt1`, the Cannonball, and the material hits.

**The bike's emitters** (`./kit fn 0x00280fb0`): engine note at +0x1494, wreck clank +0x1498, event
+0x149c, drive-on +0x14a0. There is no machine emitter, so there is no landing or suspension sound. This
is P66's finding again.

**The crowd.**
- `./kit fn 0x002afd30` reads the track's `[Crowd] CrowdPresent` into track+0x534, default 0. Every
  track's line is `; CrowdPresent=T`, commented out.
- `crowd_sound_load` (`0x001fe220`) runs only when that flag is set. It fills game+0xc0: +0x14 Ambient,
  +0x2c Cheer, +0x44 PlayerCheer, +0x5c Boo, +0x74 Oh.
- `./kit fn 0x001fe380` (crowd, kind, position) plays only when the kind's last sound has stopped:
  - 0: Oh, panned by angle;
  - 1: Boo, unplaced;
  - 2: Cheer, panned;
  - 3: PlayerCheer, an unplaced stereo stream.
- It has three callers:
  - **The wreck entry** `0x0027b810`: Oh at the chassis CofG.
  - **The place change** `0x0027edb0` (bike vtable 0x455164, thunk 0x281ad0, on the bike+0xf9c score
    card's owner interface), called from the race ranking `0x00261b80`. The ranking sorts the cards by
    +0x1c and writes the place to +0x2c.
    - On a gain, if card A's seconds ridden (+0xb4) ≥ 5.0: PlayerCheer when A's flags (+8) & 4 (a local
      human), else Cheer at A+0x60.
    - On a loss: Boo when A is human, the passer B is not, and B's place is 1.
    - Counters are kept at bike+0x12ec / +0x12f0 / +0x12f4.
  - Nothing calls it on a trick landing.

**The material collision sounds** (CollisionMaterialTable).
- The table loader `0x003ea350` stores, per material:
  - +8, `CollisionSoundFile`, loaded into +4 by `0x003ea810`;
  - +0x10c / +0x110, `CollisionMin/MaxDistance` (30 / 125);
  - +0x114 / +0x118, `CollisionMin/MaxImpulse` (5 / 25; max defaults to 1.0).
  - Bike vs. Bike Hit, ID 16, `bikehit1`, uses 2 / 9.
  - `[Settings]` holds `TerrainMask=200` and `SolidFoliageID=22`.
- The table's listener is at +0x34:
  - vt+8 is thunk `0x3eb620` → `0x003ea8a0`, which queues the probe's owner (list +0x80), the struck
    record (+0x98) and 0 (+0xb0);
  - vt+0xc (`0x3ea8f0`) does nothing.
- Interface +0x30 attaches the listener to an object (`0x3eada0`, into obj+0x1f0's listener list,
  vt +0x80) and detaches it (`0x3ead60`).
- The update `0x3ead10` runs the player `0x003ea900` when the queue is not empty. For each entry:
  - speed = |velocity| of the queued object (owner +0x80 → +0xf4 body +0xb0, else +0xf0 +0x170);
  - a repeated pair keeps the faster;
  - material = struck +0x89; a 0x10 goes through `0x0026f4b0`. When the struck +0x84 & the word at
    0x00468f48, the material is the table's +0x65 (SolidFoliageID);
  - if speed ≥ MinImpulse: level = clamp(speed / MaxImpulse × 100, 0, 100), vt+0x30 volume, vt+0x5c
    position (+0x60), `0x00104480` sound_3d with the material's distances, then vt+0x1c play(4).

## How it was proved

- **Absent sounds:** the disc dump. No file exists, and no `.aud` or `.cmt` section names one.
- **The triggers:** decompilation only. The addresses are listed above.
- **The recording:** a matched filter over S109 `context/capture/s109/run2.mp4`, decoded with ffmpeg to
  mono 22050 Hz.
  - Each clip's first 0.25 s was decoded from its VAG (Sony ADPCM, filter table
    [(0,0),(60,0),(115,−52),(98,−55),(122,−60)]) and searched by normalised cross-correlation.
  - Control: the clip injected at 0.2× the local RMS, then found again.
  - Script: scratchpad `py/xcorr.py`, not committed.

| clip | NCC | at | expected |
|---|---|---|---|
| pre_fall3 | 0.51 | 29.66 s | [Fall] index 2, tick 3726 (t = (3726−1938)/60 = 29.8 s) |
| WRECK05 | 0.78 | 30.82 s | the clank, about tick 3792 (30.9 s) |
| WRECK04 | 0.50 | 30.82 s | same attack as WRECK05 |
| FALL02 | 0.55 | 30.93 s | [IMPACT] |
| suspen00..03, trkbtm_1/2 | flat | none | no landing sound (P66) |
| drv_drt1, all 21 Impact hits | flat | none | no prop touched in this ride |

"Flat" means the top three peaks are within 0.02 of each other and at the control's noise level. The
wreck clips' second-best peaks are about 0.2.

## Not verified / not 1:1

- `./kit audit` fails on four leaks that are not this task's (atv_wheel.rs:62/66, plan/DONE.md:519/533), so the task finishes with `--skip-audit`. The P285 names add no leak.

- **Which probes carry the material listener is not pinned down.** `oracle.txt`'s
  `plane_probe_sound_listener` line calls `0x003ea8a0` the ray probe's second listener, a global at
  0x0062e324. S200's journal says a prop probe's second listener is the collection's (thunk 0x003b2b10).
  - So it is decompiled what plays once an entry is queued.
  - It is not proved which objects attach (props, the bike's own collidable, the plane's probe).
  - It is not proved whether a ridden bike hitting a fence queues, or only a knocked prop does.
- **The meaning of the mask word at 0x00468f48** (the foliage test) is a reading, not checked in RAM.
- **The prop-hit sounds were not heard.** No capture has both a knock and audio. The S195 knock states
  (s3) would need a Video Capture over AT-SPI; see the follow-up.
- **The overtake cheer and boo come from code only.** No race with a crowd exists to hear them, since no
  track sets CrowdPresent. The reading of card flags & 4 as "a local human" follows oracle's score-card
  block, unchecked here.
- **The matched filter covered one ride** (free09, S109 run2). It cannot rule out a sound that plays only
  on another map or bike. The disc listing covers that case instead.
- The per-bike engine sets (P49, S89) were not checked again by ear. This task only confirmed that each
  section is read by `BikeSounds`.

## Next

Nothing for this task. Follow-ups: F27 (the material collision sounds), C23 (the trick cheer). F27 starts by pinning which objects attach the material listener:
`./kit fn 0x3eada0`, find its callers, and settle the conflict between S200 and the oracle line.
