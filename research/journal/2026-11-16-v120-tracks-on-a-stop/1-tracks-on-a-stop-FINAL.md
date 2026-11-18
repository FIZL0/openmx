# 1 — tracks on a stop (FINAL)

Task V120. 2026-11-16. Base c5967fd79.

## What I did

- **The port's side, logged per tick.** I added a throwaway `println` to the shell's tire-mark step,
  never committed. It printed each entry's tire, gates and strip per tick. Then I rode
  `research/v40/tape.py`'s defaults (`--track free09 --play TAPE --headless --frames 195`). The end
  point is S222's: 4324, 138, 3480.
- **The original's side, on the same inputs.** `research/v40/hold.pad` is the tape's schedule in
  `./kit pad` words. `research/c15/tape.py` turns it into a from-state .p2m2, played from
  first_recording's state on copy 6 the C28 way, over AT-SPI. A scratch PINE logger read the bike, both
  wheels and both TireTrack entries every frame. It reads what `research/v40_tracks.py` and
  `research/v70/shot.py` read. At the end it read the render camera as `--shot-pose`, as V70's
  `shot.py` does.
- **A still end for one pose.** Neither bike is still at tick 195: the port creeps at ~3, and the
  original rolls at ~14. So `tape.py` takes an optional 4th argument, HOLD: that many ticks of the
  brake after the idle. `hold.pad` holds it for 90. With 0 the tape is byte for byte the old one
  (`cmp`). Every tick of `tape.py OUT 75 200 90` presses the same Cross/Square as `hold.pad`'s
  `.ticks.csv` (285 of 285).
- **Shots** (in `context/shots/v120/`, not committed):
  - `hold.png`: the original at the end of the hold tape (PCSX2 screenshot, internal resolution).
  - `port-hold.png`: the port at frame 284 from the original's camera,
    `--shot-pose 4328.92,144.90,3476.92,3.10,-17.17,85` (the mean of the camera read just before and
    just after the screenshot).
  - `side.jpg`: the two side by side.
  - `port-top.png`/`.jpg`: the port from above the launch (`4309,222,3308,0,-60,70`) at the same frame.
  - The logs: `probe.log` (port, 195 ticks), `full.log` and `hold.log` (the original, 195 and 285 ticks).
- Filed **S340**: the ride differences below.
- Nothing taken from `../opentennis`: this is a look at the ride, with no product code.

## What the original does

On V40's ride (gas 75 ticks with the bars at 200, brake 60, idle 60), the original lays two rear strips
and no front one:

| | original | port |
|---|---|---|
| rear launch strip | spinning (rider +0x930) on for ~5 ticks, laid from z≈3322, faded out by ≈3333 (alpha byte 51 → 7) | spinning on for ~35 ticks, laid from z 3313 to the lip at 3349 (20 triangles) |
| at the lip | spin again for a few ticks, one short segment (ring batch 1) | the same strip runs on to the lip |
| front | nothing | a few rows at the start and the landing (10 triangles); compression 0.67 against MinC 0.85 |
| the brake | slows to ~7, no marks | stops dead (0.2), no marks |
| after the brake | rolls on to ~14; rear spinning comes on at once and lays a strip z≈3466 → 3476, faded out over ~10 ft (alpha 51 → 8) | creeps on at ~3, rear not spinning, nothing |
| end, hold tape | 4329.7, 138.5, 3490.5 (rider +0x8d0) | 4324.2, 138.2, 3480.4 |

So **in the port, marks are laid on this ride but only at the launch.** That is about 130 ft behind and 70 ft
above where the bike stops, so they are never in the follow camera's frame. S222's shots at 45/90/120/190
showed none for that reason, and because they are faint: `port-top.jpg` shows the launch strip as a thin,
faint dark line. **The braked stop lays nothing** in either game. Neither a locked wheel (the port reads
+0xe0 as spinning, not +0xe4 locked) nor the compression under braking (≤ 0.55 against 0.75/0.85) passes
the gate.

**From the original's end pose, neither shows a mark** (`side.jpg`). The original's camera sits at
z≈3477 looking ahead (+z, pitched down 17°). Its second strip, at z 3466–3476, is under and behind the
camera, and its launch strip is up the slope behind it. It matches the port's frame, which shows no mark
either. The bikes stand ~11 ft apart (above), so the port's bike sits at the frame's lower left.

## How it was proved

- Looked at beside a screenshot: `context/shots/v120/side.jpg`, the original's end frame and the port's
  from the original's live camera. Neither has a mark in frame.
- Per-tick logs of both rides (`probe.log` port; `full.log`, `hold.log` original). The two original
  runs lay the same strips at the same frames (ring 0/32.14 at the launch, 1/32.10 at the lip,
  2/32.10 after the brake), so the movie plays back the same each time.
- `port-top.jpg` shows the port's launch strip from above.
- No code changed in the product, so no test was added. `tests/tire_tracks.rs` still proves the
  marks' rules.

## Not verified / not 1:1

- **The rides differ, so the marks differ (S340).** On the same inputs, the port's rear wheel spins
  through the whole launch where the original's spins ~5 ticks. The port's brake stops the bike dead
  where the original's only slows it to ~7. The original then rolls on with its rear spinning and lays
  a second strip that the port does not lay. Whether the shell rides the state's own bike and tuning was
  not checked; that is S340's first step. Until S340 lands, the port's marks on this ride are not the
  original's.
- **No mark was compared by eye from one pose.** The original's follow camera never frames the marks on
  this ride, and its camera can't be posed (V70). So the side-by-side shows that both frames are empty,
  not that the marks look alike. V70's launch close-up, port only, is still the only view of the marks
  themselves.
- **The original's strip positions are read off its rider point (+0x8d0) when the strip's count
  changed, not off the ring's vertices.** That point freezes in the air (it read 4312.8, 207.3, 3349.7
  all through the jump), so it is a contact point and not the body. The z ranges above are good to a
  few feet.
- **The PINE log ran at the game's pace under load and skipped frames** early in `full.log` (5 frames
  in the first 1.9 s), so the start of the launch strip is from `hold.log`, which caught every frame.
- **The speeds were not put into the same units.** The port's line prints `chassis.speed` and the
  original's prints rider +0x860. Their ratio is not constant (41.5 vs 51 at the lip, 68 vs 74 in the
  air), so the "~7", "~14" and "~3" above compare each game with itself only.
- **The software renderer was not shot.** The shots are the GPU renderer's.

## Next

Done. S340 is the ride difference.
