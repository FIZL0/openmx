# 1 — thirty second board (FINAL)

Task P375. 2026-11-18.

## What I did

- The popup the user missed is the race overlay's **30 second board**: a grey card with a big "30" on
  it, the motocross 30-second board held up at the gate. It grows in at the start's lights, turns a
  quarter and shrinks away about two seconds later. It is gone before the gate drops.
- `race_start.rs` `board_up(race)`: the seconds the board has been up, off the Sim's race. `None`
  before the lights and from the drop on. It replays the countdown listener's calls (every 0.1 s,
  its accumulator) to find the call in the lights window, as the original shows the board there.
- `ride_hud.rs`:
  - `RaceShown.board` carries it; `main.rs` fills it from `seat.sim.race`.
  - `board_look(up)` gives scale, alpha and turn by the original's three moves.
  - `board(...)` draws the card by eye in vector shapes (grey card, rim, four slots, a dark slanted
    badge with a red rule, a dark narrowed "30"), then grows and turns it about the icon's middle.
  - `Drawn.board` reports pivot, scale, turn and box for tests.
- Tests: `race_start.rs` `the_board_counts_from_the_lights_call_to_the_drop`;
  `tests/ride_hud.rs` `the_thirty_second_board_grows_turns_and_goes` (timing, place, standing then
  lying, grey card with dark "30", gone after the moves and outside the window).
- `research/oracle.txt`: four new entries (board tick, move trigger, hide, bind).
- opentennis: nothing taken. It has no race HUD and no countdown board.

## What the original does

- **The piece.** `ui\racingoverlays.lyt` (sglracui.res) has `CountdownBoard`, a UIIconComponent at
  320,88, centred both ways, 128×128. `racingoverlays1.lyr` gives it the material `30secboard`;
  `ui\tex\30secboard.pst` is 128×128: the card spans about x 24..104, y 1..102 of the icon.
- **Bound.** `./kit fn 0x2a6c60`: RacingOverlays binds its pieces by name; the board is at +0xd0.
- **Shown.** The race countdown listener (`./kit fn 0x2fb9b0`) calls the overlay show
  (`./kit fn 0x2a8560`) in the lights window [+0x4b0, +0x4b4): [0, 0.1) of the countdown, or
  [6, 6.1) with the track intro. The show makes the board visible and starts its clock (+0xe4 = 1).
- **Its moves** (`./kit fn 0x2a7da0`, the overlay tick, dt in f12). Each move runs on clock +0xe0:
  - in (+0xd4): p = e0/0.25; alpha = p·255, scale = p; done at 0.25.
  - turn (+0xdc): rotation = (e0/0.25)·−90°; done at −π/2 (anticlockwise on screen).
  - out (+0xd8): nothing until e0 ≥ 0.25; then over 0.25 s alpha and scale fall to 0.
  - then e0 += dt; with +0xe4 set, e8 += dt and the trigger (`./kit fn 0x2a8b80`, e8).
- **The trigger** fires each move once (flags +0xec/+0xf0/+0xf4), with c.ole tests: e8 ≤ 0.75 in,
  ≤ 1.5 turn, ≤ 2.25 out. Each sets e0 = 0, so a move starts on the first tick past its mark and
  plays from the next.
- **Hidden.** The drop (`./kit fn 0x2fbdd0`) calls the overlay hide (`./kit fn 0x2a85d0`) for each
  overlay of the viewport mode. It zeroes alpha, scale, turn and every board field.

## How it was proved

- **A PINE watch, frame by frame,** of C31's `c31-sx01-race-start.p2s` (sx01, intro on, length
  9.08, lights at 6.0) in PCSX2 copy 1. The RacingOverlays object was at 0x15a87a0. Read: the show
  at countdown 6.0603 with e8 0.033; scale 0.136, 0.268 … 1.0 at e8 0.334; the turn from e8 0.767,
  angle −0.214 … −π/2 at 1.068; the out from 1.501, scale 0.932 at 1.802 down to 0 at 2.035; every
  field zeroed at the drop (frame 289). `board_look` reproduces those steps on the port's ticks.
- **Looked right beside a screenshot.** PCSX2 shots (AT-SPI System → Screenshot) at board ages 0.1,
  0.4, 0.9, 1.3 and 1.9 s: `context/shots/p375/orig-*.png`. Shell shots (release build,
  `openmx --headless --track sx01 --race --shot … --shot-frame N`) at frames 4, 12, 30, 50, 70 and 100
  (drop at tick 97): `context/shots/p375/shell-f*.png`. Same place top centre, same size, same
  anticlockwise quarter turn (badge to the left, "30" reading upward), gone by the drop in both.

## Not verified / not 1:1

- **The art is drawn by eye**, not the `30secboard` texture. The HUD is vector shapes with no disc
  pictures (as the P328 flags). The badge carries no sponsor name (the original's says THQ); the
  "30" is the HUD's heavy face narrowed, not the texture's lettering.
- **Ticks of 1/30 s.** The board's clock counts the race's ticks (`CAP_30`), not the original's UI
  frame dt, so a move's start can land a tick off the original's (its out started at 1.501, ours at
  1.533).
- **The track intro is not replayed** in `board_up`: an intro's end moves the countdown on and zeroes
  the listener's accumulator. The shell counts the intro as seen, so this path is never taken today.
- **Centred on a wide view.** The original's screen is 4:3. On a wider view the board stays in the
  middle across, while the P327 lap pill (top-left pen at layout x 320) sits left of the middle; on
  16:9 the board partly covers the pill while it is up, as the original's covers nothing (its lap is
  top left).
- **Split screen not checked.** Each seat's HUD reads its own Sim's race, so each half should show
  one, as the original shows one per overlay; not shot.
- **The AllAI grid's overlay mode 6** (`hud_overlay_mode`) was not looked at; the C31 state uses it
  and the board played normally there.
- **Smoothing under scale.** The rounded shapes' edge widths are not scaled with the board, so the
  card's edge is a touch soft while it is small.

## Next

Nothing: the task is done.
