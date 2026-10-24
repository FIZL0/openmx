# 1 — backflip animation (FINAL)

Task V35. 2026-10-24.

## What I did

- **The cause.** The port's body never played its `backflip` clip (state 38). `Rider::body_frame`
  raised the request's backflip word only when the latched trick id was `Trick::BACKFLIP` (24). Nothing
  in the port ever latches 24: through a flip the trick machine's idle sets the airborne idle (25). The
  original's trick id is 25 through its flips too. So the body went to the airborne pose (35), and that
  was what the user saw.
- **The fix**, one line in `crates/game-sim/src/rider.rs`: the backflip word is now the bike's own
  turning flag, `ride.air.flipping`. MD16's front flip still shows the backflip's clip; it turns through
  the same flag, and the `Trick::FRONTFLIP` clause is kept as it was. I updated the mapping table on
  `body_frame`.
- **Captured the original's request and body state** through recording2's first two backflips, on
  PCSX2 copy 3:
  - the spec is `capture/flips.toml`, and `capture/play.sh` is V61's script with this spec;
  - the run: frame counters 2374..3473, 1100 rows, 0 torn, in `context/capture/v35/flips.csv`
    (git-ignored);
  - a second, 300-row run (`flip1-beside-video.csv`) was taken with Video Capture on
    (`flip1.mp4`). Its state and backflip word match the first run on 299 of 300 rows.
- **Two tests in `crates/openmx/tests/ride.rs`**:
  - `the_body_plays_the_originals_backflip_through_recording2` checks the original's request against
    the bike, then runs the port's body tick by tick and compares its state with the original's;
  - `a_ridden_backflip_plays_the_backflip_clip` rides P16's flip on the disc's 125 through the shell's
    sim, with the body run after each frame the way the shell runs it.

  Both fail with the old mapping and pass with the new one (checked by reverting the line).
- **By eye**: the original's flip 1 from the video, beside the port's shell riding free06's first jump
  with a backflip (`--play` of a generated tape, the memory's `--trace-stick-y` schedule with Cross
  held). The crops are in `context/shots/v35/orig-vs-port.jpg`, with the tape and its generator.
- `research/oracle.txt` has a V35 note under S27's request-block fields.

Nothing from opentennis applies here: it has no rider animation state machine to take.

## What the original does

- The request block the body's selection reads (S27's `+0xf8`/`+0xfc`/`+0x1a0`) is filled, every
  tick, from the bike's words of the same tick:
  - backflip `+0xfc` is the bike's `flipping` (`+0x4f8`, which `./kit fn 0x00256b50` sets with the
    torque each substep). They match on **1100 of 1100** rows, rising and falling on the same tick;
  - airborne `+0xf8` is the bike's `+0x57c` on every riding row;
  - the gesture slot `+0x1a0` is the trick id `+0xa78` when it is below 24, and 24 otherwise.
- Flip 1, as an index (frame_counter):

  | Tick | What happens |
  |---|---|
  | 2507 | the flip triggers (`+0x4fc`) |
  | 2512 | airborne, so state 35 |
  | 2513 | the first tick turning: request backflip up, state 38 `backflip`, `holding` raised, ragdoll unplaced (Posed) |
  | 2544 | a Taunt (slot 0) asked mid-flip outranks it, so state 0 |
  | 2581 | the turning ends; the Taunt plays on to 2594 |
  | 2595 | state 35 |
  | 2596 | landed, so state 30 |

- Flip 2: the trick (slot 1) is asked at 3301, a tick before the turning starts at 3302, so the
  gesture arm wins and 38 never runs. The turning ends at 3380, then state 35 at 3388 and 30 at 3418.
- The body's want and state are equal on every row, so the step never held a state back.
- The clip itself is played at a fixed tenth of its length (S25's step, already ported), so the
  backflip is one held pose while the bike turns under it.

## How it was proved

- **A PINE recording compared tick by tick.**
  `the_body_plays_the_originals_backflip_through_recording2` reads `context/capture/v35/flips.csv`.
  On each of the **976 riding ticks** (the wreck's ticks are left out):
  1. it asserts the original's request is the bike's words, as above;
  2. it sets the port rider's `air.airborne`, `air.flipping` and latched trick id from the row and runs
     `Rider::body_frame`. The port's (want, state) equals the original's on all 976 ticks, and the
     backflip runs on exactly 2513..2543.

  With the old mapping, 31 ticks fail: the port is (35, 35) where the original is (38, 38).
- **The shell's own ride.** `a_ridden_backflip_plays_the_backflip_clip` checks four things:
  - the body is in `backflip` on exactly the frames the bike is turning (frames 171–277 of the
    script);
  - it is in the airborne pose on every other airborne frame, including the frame the rotation ends;
  - it rides on every grounded frame after the landing;
  - the flip does not wreck.
- `./kit check -- -p openmx --test ride -- backflip`: 3 tests passed. Below, the full gate.
- **By eye** (`context/shots/v35/orig-vs-port.jpg`): the top row is the original at about 4.6, 4.8,
  5.0 and 5.3 s of `flip1.mp4`; the bottom row is the port at 30 Hz ticks 252, 262, 272 and 284 of the
  free06 flip. Mid-flip, both riders are tucked to the bike with the arms bent out to the bars. The
  tracks, camera angles and rotation phase differ, so it is a likeness, not a match.

## Not verified / not 1:1

- **The writer of the request block is still not located.** The mapping is proved by values: equal
  on all 1100 captured ticks, from one recording. The code that copies `+0x4f8` into `+0xfc` was not
  found, so any condition it adds that this capture never hit (a remote rider, a replay, the biplane)
  is unknown.
- **Only one flip shows the clip.** recording2's flip 2 is hidden by a trick asked a tick earlier,
  and flip 3 (4077, which wrecks mid-flip) is past the capture's end. So 38 is proved over one span
  of 31 ticks, and a flip that lands while still turning was not captured.
- **The test hands the port the original's bike words.** It does not ride the port's own bike
  through recording2: the rides diverge. That `air.flipping` is the original's turning flag tick for
  tick was proved by S31 (`the_humans_backflips`), with its four known stick-timing misses, not here.
- **The wreck's ticks are left out** (3016–3139, plus the request's tick behind it). The test body
  is not thrown, so it has no wreck to follow, and the wreck states (40, 42) are not compared.
- **The clips' lengths and fade rates in the test are its own** (one second, no loop; `Rider::new`'s
  zero fades, as S27 left them). The comparison is of the states asked for and run, not the pose
  sampled or the cross-fade's weights.
- **The look is by eye only, and not the same moment.** The port's shots are free06 from a
  generated tape and the original's are free09 from recording2's video. The camera, the rotation
  phase and the clip's tenth are not aligned. The video's frame↔tick offset was not measured; the
  tick numbers given for the video are estimates.
- **`./kit shot` failed on this copy.** The window grab fell back to the hotkey, which wrote nothing
  ("PCSX2 wrote no screenshot"), so the original's pictures come from Video Capture instead.
- **The riding ragdoll is still off by default** (`OPENMX_RIDING_RAGDOLL`). With it on, a backflip is
  Posed (from 2513 the original unplaces the ragdoll), and V61's gate handles that. I did not look at
  it in the shell.

## Next

Done as briefed, with the gaps above. C8 (a recording with a flip and a trick) would give a flip
whose clip is not cut short by a trick; once it lands, add its ticks to the capture test.
