# 2 — hud redo, carried onto main and checked on PCSX2

Task P121. 2026-10-23.

The first attempt (entry 1, branch wip/P121) did not merge: main had moved 27 commits on (V52's HUD
scale and Minimap, P126's Original tab, P70), and the merge did not build. This entry carries that
commit onto main, fixes what broke, wires what was missing and settles the land/bail colour on the
original's own frames.

## What I did

- Cherry-picked wip/P121 onto task/P121 (main at 47b8921). Conflicts: main.rs's `--help` (kept V52's
  `--minimap`/`--hud-scale` and `--tricks`, with `hud|original|debug|none`), plan files (main's).
  The build broke on two `Settings::hud_text`s (V52's minimap/scale lines and P121's `hud = 1` line):
  P121's is `ride_hud_text` now.
- `main.rs`: `ride_hud::Shown`'s `scale` and `minimap` come from V52's settings (`overlay::Art`'s
  `scale`/`minimap`, which the ride loop sets from Settings or `--hud-scale`/`--minimap`) instead
  of entry 1's fixed 1.0 / true.
- `menu.rs`, P126's Original switch: the HUD is now a departure like the others. Off puts an
  Original overlay back to Hud (Debug, None, the radar and the scale are left alone); On still puts
  Original, minimap on, scale 100. P126's tests said a fresh file's HUD "starts as the original's"
  and "Off leaves the HUD as it is" — both written when the original's HUD was the default; they now
  say the HUD starts as the port's and Off brings it back.
- `ride_hud.rs`, the pop-up's timing, from the film below: a new name grows in from half its size
  (POP 0.15 s; entry 1 had it shrinking in from twice its size), and an ended combo shrinks to 0.2 while
  fading (OUT 0.25 s; entry 1 had it growing to twice). A landed chain of two or more shows the
  original's `N Stunt Combo Total` over the banked total, held a second, in place of entry 1's
  `+banked`; a single landed trick and a bail play out at once, the bail keeping its `xN`.
- Tests: `tests/ride_hud.rs` follows those (the total line, shrink-out, grow-in) and a new one checks
  that HUD scale 150 grows every piece about its corner and Minimap off drops only the radar;
  `tests/menu.rs` the two P126 assertions above.

## What the original does (filmed)

`research/journal/2026-10-22-v53-preload-meter-fill/capture/play_with_video.sh` (V53's recipe) on
copy 5: recording2 played back from frame 2374 for 1,260 ticks with System → Video Capture
running (`context/shots/P121/recording2.mp4`, PINE csv `context/capture/p121/recording2-popup.csv`,
both git-ignored). The acceptance csv's columns show what is in it: a trick landed near frame 2595
(BackFlip + Taunt, then BackFlip ×2), a single trick (CanCan), BackFlip + Lazyboy, and at 3522–3562
a trick running into a wreck (wreck_reason 12, trick_count 1) — the bail.

- Colour: the popup's bright, saturated pixels average (186–190, 111–115, 9–11) on the video in
  every frame of the two-trick land, its total and the bail — the orange (255,153,0) through
  the capture's levels. No green pixel in the popup's box in any frame of the film, and the few
  red ones (53 at most) are the rider's red kit passing through it (`land-zoom.jpg`,
  `land-out.jpg`, `bail-zoom.jpg`, `single.jpg`).
- Land of a chain: the chain line goes, `2 Stunt Combo Total` / `24160` (= (10000 + 2080) × 2)
  grows in at the same place, stands about a second, then shrinks toward the middle and fades in
  about 4 video frames at 15 fps.
- Bail: `Taunt` / `3330` shrinks and fades the same way the moment the rider goes down, no word.
- This agrees with entry 1's reading of the HUD listener (research/oracle.txt, P121), which now has
  frames behind it.

## How it was proved

- `./kit check` through `./kit done` (the verdict is quoted in the commit, not a count).
- `cargo test -p openmx --test ride_hud --test menu`: all pass (29 + 131).
- Shots in `context/shots/P121/` (git-ignored): `ride-shot-1080p.png` is the shell's own `--shot` at
  1920x1080 (free06, gas, d-pad Right at tick 100, shot at 115: score bar, radar, DSEG7 37 by the
  preload meter, the marker menu on slot 2); `ride-composed-1080p.png` is `ride_hud::draw` over the
  same ride's `--overlay none` frame with a two-trick combo, the menu holding Down halfway with Ann's
  pill and the Biplane prompt; `pcsx2-combo.png` the original with BackFlip + Taunt up (from the
  film); `side-by-side.png` concept · port · original.

## Not verified / not 1:1

- The pop-up's timings (POP 0.15 s, HOLD 1 s, OUT 0.25 s, grow from 0.5, shrink to 0.2) are by eye off
  a 15 fps sheet of the video, not read from the Stunt movie's frames.
- The original's `xN` is in the same orange as the points; the port keeps entry 1's pale multiplier
  ink.
- The bail filmed was a single trick; a bailed chain of two or more was not filmed (the port plays it
  out with its names and `xN`, as the single one).
- Original-switch Off turning Original back into Hud is my call, not the user's: P126 said Off leaves
  the HUD alone, but that was when the original's HUD was the default and so "ours".
- The shell's own shot still has no combo and no prompt: the shell's rider never scores on the taped
  ride (P129), and free06's spawn offers no machine switch; both are in the composed shot, drawn by
  the same code fed by hand.
- Everything else in entry 1's list stands except two items it no longer applies to: HUD scale and
  Minimap are now wired from V52's settings, and the PCSX2 picture is a new frame with a combo up.
