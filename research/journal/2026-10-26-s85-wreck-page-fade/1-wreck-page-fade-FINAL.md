# 1 — wreck page fade (FINAL)

Task S85. 2026-10-26.

## What I did

- Added `wreck_page::Shade` (game-sim, float-linted). It is the fader's picture side. It follows a
  `Page` one tick at a time, gets the tick's frame time in whole ms, and gives the black's alpha.
  The simulation never reads it, and it is in no hash or snapshot. So no state pin moved.
  - `Shade::update` is the original's update: elapsed += ms * 0.001, then t = min(elapsed / 0.1, 1).
  - `out_alpha` / `in_alpha` are the two draws: `(int)(0.5 + 255 t)` and `(int)(0.5 + (255 - 255 t))`.
- The `Page` still decides when black comes: it is the third update. `Fade::In` with no updates
  draws 255, and `Fade::Off` after the fade in draws 0.
- Shell (`crates/openmx/src/main.rs`):
  - Each `Seat` holds a `PageShade`. It is stepped at the end of `Seat::pass`, after the bike, as the
    original steps its fader.
  - `PageShade` has a millisecond clock: the passes' `dt`s summed in f64. Each pass gets the whole
    ms the clock moved, which is 33, 33 and 34 at 30 Hz.
  - The GPU path draws a plain black quad over the view first in the view's sprite list
    (`menu::ink::fill`), so the HUD sprites go over it. The software path darkens the view's frame
    after the scene and before the HUD (`shade_view`).
  - Each change prints `page: seat N alpha A`.
- Took nothing from opentennis: it has no viewport fader.

## What the original does

- The fader's update (`./kit dis 0x001c8a80 --len 0x60 -p boot`) only runs while t < 1 and the
  duration is not 0. It adds the dt argument ($f12) to elapsed (+0x34), then sets
  t (+0x3c) = min.s(elapsed / duration, 1).
- The fader's draw (`./kit dis 0x001c8f50 --len 0x100 -p boot`):
  - Fading out (+0x48): alpha = cvt.w.s(0.5 + 255·t). At 255 it goes to the black/put-back path
    (S72).
  - Fading in (+0x4c): alpha = cvt.w.s(0.5 + (255 − 255·t)). At 0 it goes to the let-go path.
  - Held only (+0x54): 255.
  - The colour is black (+0x68, S57).
- dt is whole ms as seconds. The live elapsed bits match `ms * 0.001`, not `ms / 1000.0`: north
  row 77's 0.066 + 0.034 is 0.099999994 in the original. The division gives 0.1.

## How it was proved

- `crates/game-sim/tests/wreck_page.rs` `the_originals_fade_steps_by_whole_milliseconds` runs on
  S72's three live captures (context/capture/s72/{north,east,west}.csv, copied into this slot from
  s5).
  - Every row where the fader is fading out or held is checked. `Shade::update` gets the whole ms
    since the row before, or two updates of 33/34 ms in whichever order matches the bits, where the
    row before was read ahead of its tick. The elapsed and t it leaves are bit-equal to the row's.
  - That covers 37 live updates, every step 33 or 34 ms.
  - On those same live t values, `out_alpha` reaches 255 exactly on the third update out (3 rows
    seen), and `in_alpha` reaches 0 exactly on the third update in (6 rows).
  - With all three captures present the test asserts these counts as minimums, so a silent skip
    cannot pass on data that is there.
- `the_shade_follows_the_page` pins the alpha sequences for two orders. 34/33/33 draws 87, 171,
  255, 168, 84, 0. 33/33/34 draws 84, 168, 255, 171, 87, 0. A tick with no page update draws the
  same as the tick before.
- Shell unit tests (`page_shade` in main.rs) check that 30 Hz passes give 84, 168, 255, 171, 87, 0,
  and that `shade_view` darkens the view.
- Live in the shell, by eye: a hand-written tape (gas held, free06's first jump, the stick pulled
  through the landing) wrecks at about 11 s (reason 8). The page prints 84, 168, 255, put back,
  171, 87, 0.
  - GPU shots at ticks 422–430 have mean brightness 85, 57, 29.5, ~1 (only the HUD left), 29, 57,
    85. That is 85·(1 − alpha/255) on each tick.
  - The software renderer darkens the same way.
  - The shots are in the session scratchpad, not committed.

## Not verified / not 1:1

- **The alpha itself was not read live.** The capture holds the fader's elapsed and t, not the
  alpha it drew. The formula comes from the draw's disassembly, and is applied to live t values.
- **The port's milliseconds are the shell's own clock, not the PS2's real frame time.** The
  original's frames are 33 or 34 ms, depending on where its ms timer falls (the captures show
  33/33/34, 34/33/33 and 34/33/34). The shell's clock gives 33/33/34 at 30 Hz, and a tape gives its
  own dts. So a given page may draw 84 where the original drew 87, or 168 where it drew 171. The
  black and the clear ticks are the same either way.
- **Black follows the Page, not the shade's t.** With odd tape dts (three passes summing under
  100 ms), the original would not be black yet. The port still draws 255 and puts the bike back on
  the third update (S72's modelling of the put-back).
- **Over or under the HUD.** The black is drawn over the world and under the shell's HUDs, by eye.
  The order of the original's viewport fader against its own overlay was not checked.
- **Between ticks.** Above 30 fps the picture holds the tick's alpha until the next tick. It is
  not interpolated, as the original's draw is once a frame.
- **Online rollback** re-runs `Seat::pass` for resimulated ticks, and the shade (presentation
  only, not rolled back) steps again with them. The page's own transitions keep it right, but the ms
  clock moves extra, so the 33/34 order may change. This was not run online.
- **A page restarted while one runs** (the original's fader_start, which reverses a fade in) is not
  modelled: the shade restarts from 0. The machine never asks for one while a page is up (S72).
- **Split screen** uses one shade per seat, over that seat's view (S72: one page per rider).

## Next

Done.
