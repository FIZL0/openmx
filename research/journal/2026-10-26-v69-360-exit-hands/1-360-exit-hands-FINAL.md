# 1 — 360 exit hands (FINAL)

Task V69. 2026-10-26.

## What I did

- Re-ran V64's 360 in `crates/game-render/tests/whip_tricks.rs` with the main checkout's `mods/tricks`
  copied in (git-ignored). **It was red on HEAD**: "the body played threesixtyR for 0 frames". MD30
  (c0c8997c) took MD25's one-push easy 360 out after V64 landed, so V64's single push right no longer
  fires a 360. A worktree without `mods/tricks` skips the test and passes, so the gate never showed it
  (memory: leftover-mods-folder-reds-whip-tricks).
- Rode MD30's gesture instead (the M21 hands from `game-sim/tests/ride.rs`): R1 from 130, stick left at
  150, right at 158, centred at 166, everything let go at 180. The body plays `threesixtyR` (69) from
  182 to 267, fades back to the airborne row (35) from 268, and lands at 304. Unheld, the clips take a
  hand up to 0.81 ft off its grip on the exit (frame 275), as V64 measured.
- Then measured what the shell draws, not the bare clip pose. Both renderers draw the rider through
  `Body::palette` / `Body::skin_into_at` with the chassis' live grips, and `hold_limbs` moves the arms onto
  them at `Body::holds`. The 360 is not a pose off the clip table, so the body's mode is `Riding` through
  the clips, the exit and the landing. The hold is 1 and **the drawn hands are 0.000 ft off the grips
  on every frame**. The 0.82 ft that V64 filed as V69 is the clip pose before the hold. The shell never
  draws that pose (unless `OPENMX_RIDING_RAGDOLL=1`, below).
- No code change was needed. The test now covers it: `a_360_keeps_the_hands_on_the_bars` (new) asserts
  that the hold stays full, that the drawn hands stay within 0.01 ft of the grips from the 360's first
  frame to the landing, and that the clips alone still part them by more than 0.5 ft inside the exit's
  fade, so the test proves the hold is doing the work. V64's own test lost its broken 360 half and
  keeps its three tricks. Mutation: with `Body::hold` forced to 0 the new test reds "the 360 at 182: the
  drawn rider lets go of the bars (state 69)".
- Took nothing from opentennis: it has no limb hold or tricks.

## What the original does

MX vs. ATV Unleashed, since MX Unleashed has no 360. I filmed it in PCSX2 in my own copy 1. The scratch
config was this slot's kit.toml with the iso set to openmx-mods' `MX vs. ATV Unleashed (USA).iso`,
serial SLUS-21104, and openmx-mods' vsync word. I copied the save state into copy 1 for the run and
removed it afterwards. The state is M21's state 4, copied from `ps2-decomp-kit-slots/pcsx2/s9`. The
driver is M21's closed-loop `both.py --move spinpull`
(`~/repos/openmx-mods-slots/s1/extracted/mxvatv/watch/both.py`, its `sys.path` pointed at this repo's
ps2kit). PCSX2's Video Capture ran over AT-SPI at the same time: `capture/film.sh`, P49's press.py and
save_as.py.

- The run matches M21's: armed at vsync 166, fired at 170, take-off at 202, the 360's turning rule from
  218 to 286, a clean landing at 298, and "Three Sixty Right" scored.
- In the 360 the rider's body swings out sideways off the seat, legs out. On the exit it swings back
  onto the seat, and **both hands are on the bar ends the whole way back**, as far as 640x480 shows
  (frames at t 5.15–5.6 s of the video). This is the riding ragdoll's hold pins (V61) holding through a
  360, which agrees with the port's mode rule: the 360 is riding, not posed.
- Shots (not committed): `context/shots/v69/mxvatv-360-exit.jpg` (six exit frames, 12 fps, zoomed),
  `mxvatv-360-whole.jpg` (the whole jump at 10 fps), `spinpull.csv` (the driver's log).

## How it was proved

- `whip_tricks.rs` `a_360_keeps_the_hands_on_the_bars`, port only. It covers 122 frames from the 360's
  first frame to the landing, with a mutation that reds.
- The original was checked by eye beside one PCSX2 video capture of MX vs. ATV's 360, with nothing
  measured in its RAM.

## Not verified / not 1:1

- **The original's hands were judged by eye only, not measured.** I read no bone or ragdoll body
  positions from MX vs. ATV's RAM, because its rider and ragdoll layouts were never mapped (M19–M21
  mapped the bike only). At 640x480 with a chase camera, an error of a few inches in the hand would not
  show. What can be seen is that the hands do not come off the bars as a 0.8 ft clip-pose error would.
- **The original is MX vs. ATV**, the source of the pack's 360. MX Unleashed has no 360, so the trick is
  not in the target game at all.
- **One run was filmed**: state 4's first lip, the `spinpull` hands (the stick held back in the air).
  The plain `spin`, which lands part way round and wrecks, was not filmed. The landing frames were not
  compared either.
- **Not checked with `OPENMX_RIDING_RAGDOLL=1`.** That path draws the riding ragdoll, whose pins hold
  the hands with the ragdoll's own compliance. The test checks only the default clip-plus-hold draw.
- **The port's 360 clip plays a second time just before the landing** (69 from frame 289 to 304). The
  pack's clip plays while the heading is more than 35° off take-off (rider.rs `spin_state`, MD21), and
  on this generated ramp the port's 360 turns past 395°. The original's spinpull turned 365° and landed
  9° off, with no second swing in the video. That is the 360's turn and MD21's clip rule, not the hands
  (they are held through it too), so I did not chase it here. Filed as MD35.
- The test needs `mods/tricks` (git-ignored). Without it the test passes silently, as V64's did, and
  that is how V64's 360 half stayed red unseen. I copied main's pack in to run it here.
- The arms' shape while the hold bends them to the bars (elbow angle, how the bend looks) was not
  compared with the original. Only where the hands end up was checked.

## Next

none
