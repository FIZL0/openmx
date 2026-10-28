# 1 — tire tracks beside (FINAL)

Task V70. 2026-10-27. V40's follow-up: the marks beside the original, the software renderer by eye,
and the zone byte the tracks gate on.

## What I did

- **The zone proof** — `crates/game-sim/tests/wheel_zone.rs`. The capture comes from the human's
  animandpump, played back on the original with `watch/v70-zones.toml`. Per wheel it holds the
  surface byte (wheel +0x110), the ray's end (+0x160), the hit word and the load. It also holds the
  snapshot's zone bytes (rider +0x910/+0x911) and the particle manager's per-tire zones. The test
  asks the port's `World::wheel_surface` on free09's disc terrain at the original's ray end for each
  wheel and row.
- **The capture recipe** — `research/v70/play.sh` (copy 2, under `./kit pcsx2 with --`) is C6's
  `play_capture.sh` rewritten for this spec: animandpump's chain (`... 0x34 -> 0x8 -> 0x0`) and a
  `frame_counter` start window:
  `./kit pcsx2 with -- bash research/v70/play.sh <PID> context/recordings/animandpump.p2m2 1939 1100 context/capture/v70/animandpump.csv`
  It gave 1100 rows, frames 1939..3038, none torn. The .p2m2's md5 was unchanged. The file is not
  committed; it lives in s2's context/capture/v70/.
- **The shots** — `research/v70/launch.pad` is a hard launch from first_recording's free09 spawn:
  0.9 s full gas straight on, then the brake for 3 s to a stop. It then backs up on an arc
  (gas+brake, bars full over, 2.5 s) so the bike stands beside its marks, and holds the brake 6 s.
  `research/v70/shot.py` loads the state into slot 8 and plays the pad, logging speed, the rear
  wheel, the zones and each TireTrack entry's strip/ring every 0.25 s. It then reads the render
  camera and its projection, prints them as `--shot-pose`, and takes `./kit shot`. With
  `NOTRACKS=1`, or a third argument, it also takes the same ride or frame without marks.
  `research/v70/tape.py` writes the port shell's tape with the same schedule.
  - Port shots: `openmx --headless [--renderer software] --track free09 --play launch.tape --frames 445 --shot-frame 440 --shot-pose <pose> --size 640x448`.
  - Copy 2's PCSX2.ini had ScreenshotSize set to 1 (internal resolution). Before that, shots shrank
    with the window: 936 → 593 → 456 px.
- **Follow-up S183**: the shell's reverse barely steers at full lock (below).
- **Note in research/oracle.txt**: the zone bytes are the wheels' surface bytes.

## What the original does

- **The zone bytes** — `./kit fn 0x00258d50` (the stunt-state pack) copies each wheel's surface byte
  (front wheel at rider +0x7b0, rear at +0x7b4, byte +0x110) into the snapshot at +0x910/+0x911.
  `./kit fn 0x0027adf0` points the manager's tire records at that snapshot. The TireTrack entry's
  zone gate (V40) reads it there. The port's tracks take `ride::Wheel::surface`, which is the same
  byte, so the same zone reaching both comes down to the port's terrain query answering the
  original's byte.
- **The manager's own copy** (+0xf64, then +0x29 front / +0x55 rear) is the snapshot's zone of this
  frame or the one before. On one reset tick (2996: speed 0, the body dropped ~10 ft) it read 0xff
  for both wheels.
- **The launch marks** — the rear strip lays about 14 rows, from z≈3319.5 to 3334 at x≈4309. The
  segment alpha bytes are 51, then 42 / 17, then a later segment at 6. They are faint, hardly
  visible at 640x480 from the chase camera.

## How it was proved

- **Zone byte: a PINE recording compared row by row** — 1100 rows × 2 wheels, `wheel_zone.rs`
  (`./kit check -- -p game-sim --test wheel_zone -- --nocapture`: "1100 rows, surfaces {0: 336,
  13: 1864}, 2 manager zones held at 0xff").
  - Snapshot zone == wheel surface on every wheel row.
  - The port's `wheel_surface` at the original's ray end gives the original's byte on all 2200, with
    the road's 0 and the dirt's 13 both ≥100 times. Animandpump crosses free09's road at ~2737–2910
    (x≈4433–4478, z 3759–3896).
  - The manager's zone is the row before's or the row's own, apart from the two 0xff readings.
- **Marks: looked at beside a screenshot.**
  - `context/shots/v70/pose_orig_port.png` puts the original's frame and the port's from that same
    pose (4335.950,214.965,3340.788,-141.347,-18.229,85) side by side. There are no marks you can
    point to in either. The original's darker streak where the strip projects (x≈460–575, y≈205–230)
    can't be told from the ground texture.
  - `context/shots/v70/top_gpu_software.png` is the port's marks from 14 ft above
    (4309,222,3312,0,-60,70) in the GPU and software renderers. It shows one strip in the same place
    and at the same length in both, and as faint as the original's alpha bytes suggest. The software
    renderer keeps the tread's chevrons sharp where the GPU smooths them. That is a texture-sampling
    difference, matched by eye per AGENT.md, so no follow-up.

## Not verified / not 1:1

- **The marks were not compared pixel for pixel with the original's.**
  - Launch marks this faint can't be seen at 640x480 from the chase pose in either game.
  - The original's camera can't be posed overhead the way `--shot-pose` poses the port, so the close
    view exists for the port only.
  - A same-frame diff with the rings emptied (`shot.py`'s third argument) was swamped by camera
    drift and a pylon popping in at LOD.
  - A second, mark-free ride (`NOTRACKS=1`) parts from the first by ~10 ft, because the pad is
    played by wall clock.
- **The two games' bikes end in different places in the shots.** The forward launch matches (same
  line, similar stop), so the marks lie in the same place. The backing-up arc does not match: the
  original turns ~140° at full lock, while the port backs nearly straight (S183). The pose shot
  therefore shows the bike in a different spot.
- **The 0xff manager zone on the reset tick (2996) was not traced.** The test allows it, at most twice.
- **`wheel_surface` was checked only on free09's terrain (dirt 13 and road 0).** Other surface ids,
  other tracks, and props or objects under a wheel (the query is terrain only, S89) were not covered.
- **The spinning flag (+0x91c/+0x930, V40's slip.spinning)** was logged in shot.py's lines but not
  checked against the port in a test.

## Next

- S183: the reverse full-lock arc.
