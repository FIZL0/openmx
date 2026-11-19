# S482 — the landing lag on the generator

2026-11-19. Task: `./kit plan S482`.

C33's generator parted at tick 86 because the original drew once more than the Sim on tick 85. That draw is
`ai_habits`' landing-lag roll (0x00280420, ra 0x00280428). It needs the card's air time (+0xfc) to reach the
hang time (+0x110c, 1 s). In a free ride, the score keeper's jump watch (`score_mode3` 0x00268d00 →
`score_card_jumps` 0x00267d90) writes that air time and the ground clock (+0x1b0). It runs after every rider,
on cards with WATCHES_JUMPS. The Sim never ran it, so the habits' `air_time` and `lag_clock` stayed put.
That port is now `Pilot::watch_jumps`, called each tick by `Sim::watch_jumps` from `step_all` (after the race
frame) and from the flown path, on free rides only. The generator now agrees through 130 (was 84).

## How it was checked

- draws.rs (S328) replays the frames to tick 132 (it stopped at 63 before). Per tick it now checks:
  - the dust and track sites drawn;
  - which pilots roll the landing lag: 85 ai2, 94 ai1, 95 ai0. None wins the roll.
- race4.rs (S12e four racers) marks are now `[318, 303, 1802, 278, 130]` (were `[296, 302, 1802, 280, 84]`).
  The Sim's dust is held to the capture through 131 (it was 63).
- The whole `ai` test target passes (77), and so do `snapshot` and `race`.

## Not verified / not 1:1

- At 131 the original makes 7 draws (one engine note, six dust) and the Sim makes 6. At 132, ai0's foot dust
  draws in the original only. Filed as S482a.
- ai2's mark fell from 280 to 278. That is from the draws after the generator parts at 131. With the generator
  put back to the capture's value each row (a throwaway test run), ai2 agrees to 729. S482a should bring it
  back.
- Sim races (laps) still never move the habits' air time or ground clock: B38.
- The jump watch while a rider is down after a wreck (the card dropping WATCHES_JUMPS) was not checked against
  a capture. C33 has no AI wreck in the window.
