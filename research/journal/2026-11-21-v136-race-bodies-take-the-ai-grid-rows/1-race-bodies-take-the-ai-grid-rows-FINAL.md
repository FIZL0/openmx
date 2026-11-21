# 1 — race bodies take the ai grid rows (FINAL)

Task V136. 2026-11-21.

## What I did

- `Body::load_in(…, ai_out)` (crates/game-render/src/rider.rs): with AI riders out the set keeps the
  archive's claim order (`airider.mcf` first, V14); without them it calls `Set::alone()` as before
  (V121). `Body::load` is `load_in(…, false)`, so every other caller is unchanged.
- crates/openmx/src/main.rs: `race_puts_ai_out(racers, seats, online)` says before the grid is laid
  whether the race has lanes past the seats that AI riders take (offline, or online with one seat).
  The first body, the other players' bodies and a mid-ride join use it; the race AI bodies pass
  `true`; an online peer passes `false`.
- Tests: `a_body_beside_ai_riders_binds_the_ai_sets_grid_rows` (crates/game-render/tests/body.rs, disc)
  and `race_ai_out::a_race_with_lanes_past_the_seats_puts_ai_riders_out` (main.rs, unit).

## What the original does

One global animation set (oracle.txt `anim_set`); a clip name's first claim fixes its file (V14,
`./kit fn 0x00142220` the .mcf reader, `./kit fn 0x001426a0` the lookup by name). Once an AI body is
built, `airider.mcf`'s claims stand for every body after it, the human's included:

| name           | airider.mcf          | mrider.mcf (own)     |
|----------------|----------------------|----------------------|
| turn_stop_Fwd  | turn_stop_Mid.MOT    | turn_stop_Fwd.MOT    |
| turn_stop_Bck  | turn_stop_Mid.MOT    | turn_stop_Bck.MOT    |
| turn_still_Bck | turn_still_Bck.MOT   | turn_still_Mid.MOT   |

## How it was proved

A save-state capture, read as RAM images: which grid `.mot` files the original loaded. For each of
the six grid files dumped off thq1.res, the script finds 32-byte windows that occur in no other grid
file and counts their copies in each image (scratchpad `present.py`, `python3 -I present.py
context/states/<stem>`; the guard needs the stem).

| image (context/states)                     | AI out | stop F/M/B | still F/M/B |
|--------------------------------------------|--------|------------|-------------|
| c31-sx01-ai4 (sx01, 4 racers)              | yes    | 0 / 3 / 0  | 1 / 1 / 1   |
| c40-sx17-race-start                        | yes    | 0 / 3 / 0  | 1 / 1 / 1   |
| s282c-sx01-race                            | yes    | 0 / 3 / 0  | 1 / 1 / 1   |
| v136-sx01-ai4-noallai (sx01, 4 racers, AllAI 0) | yes | 0 / 3 / 0 | 1 / 1 / 1   |
| s196-sx01 (S133 boot, sx01, 1 racer)       | no     | 1 / 1 / 1  | 1 / 2 / 0   |
| s196-free11, s133-free10, c34-free06-boot, r52-free07-baja, s342-baja-free07, c14-kiowa, s340-ball-772, s296a-sx06, s296a-nat03 | no | 1 / 1 / 1 | 1 / 2 / 0 |

In c31 the six loads sit one after another, ~0x1570 bytes apart, in grid order (stop_mid three times
at 0x11d2ac0, 0x11d4030, 0x11d5600; then still fwd/mid/bck). The race images are saved at the gate
before the countdown, with the human stopped there: the stopped rider's grid rows are the AI set's.

C31, C40 and S282c boot with AllAI on, which hands the player's bike to the AI too. To rule that
out I booted sx01 with 4 racers and AllAI 0 (`./kit pcsx2 with -- python3 research/c31/boot.py 41 4
1 0`, then `./kit state ram 41 --out context/states/v136-sx01-ai4-noallai.ee.bin`; the image holds
AllAI 0 and NumberOfRacers 4). It loads the same AI column, at the same addresses as C31's.

## Not verified / not 1:1

- No per-tick pose capture of a stopped rider in a race: the proof is which files the RAM images
  loaded, not the drawn pose. No joint capture spec covers a race actor list.
- A race with no AI riders on a track with a gate (S133's sx01 with one racer) shows the player's
  column; a race on a track with no gate was not imaged. The port asks for the gate's two inputs
  (start calls, drop-gate prop) before the first body and prints a line if its guess and the laid
  race disagree; that line was not seen in a run here.
- The original's spawn order (that an AI body is built before the human's) was not read; the RAM
  images show only the result.
- Online peers are taken to race with no AI rider out (AI riders race only online alone).
- The AI bodies still use the player's model, as V133 left them.
- The garage and trick views still load alone (no race there); the original's menus were not read.

## Next

- Nothing for V136. A per-tick joint capture of a race actor list would let a stopped grid pose be
  compared bone by bone.
