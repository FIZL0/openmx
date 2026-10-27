# 1 — controls pad for pad seat (FINAL)

Task P178. 2026-10-26.

## What I did

- The bug: with one seat the Controls tab drew the keyboard list under Button Layout whenever
  `!many_seats()`, never asking what 1P rides. With several seats it drew the keyboard under the Player
  row for every seat.
- `PadSlots::on_keys(i)` (pad.rs, with `Seats::on_keys` passing it on): seat `i` is the keyboard's when
  the keyboard drives it (`keys()`) *and* no pad holds it. Without `--pads` `keys()` is `Some(0)` even
  with 1P's pad in (the keyboard rides alongside it), which is why `keys()` alone would not do.
- `Menu::keys_seat`: the player on the keyboard, filled every frame by the front end (pad seats) and by
  the ride for its pause (ride seat `i` → `args.pad_seat(i)`). `None` without a shell's seats.
- `Menu::controls_on_keys()`: the Controls tab's seat (`control_seat()`) is `keys_seat`. menu_screens.rs
  draws the keyboard only then; otherwise the row falls through to the pad diagram, as the other rows do.
- Nothing from opentennis: its controls page has no keyboard/pad split like this.

## What the original does

Front-end only; the original has no keyboard. Ours to design (AGENT.md).

## How it was proved

- `pad::tests::a_seat_with_a_pad_is_not_on_the_keyboard`: default seats with a pad in 1P → not on keys
  (the user's case); pad out → on keys; keyboard-1P from the splash → seat 0 only.
- tests/menu.rs `the_controls_tab_shows_the_keyboard_only_to_its_rider`: one seat and two, keyboard on
  1P, on 2P, on none.
- Headless shots of `--menu --screen controls`: with no pad (this machine) the 1P KEYBOARD list as
  before; with `keys_seat` forced to `None` in a throwaway build, the pad diagram under the layout
  choices, fitting the panel at 1920x1080.

## Not verified / not 1:1

- Not seen with a real pad plugged in: no pad reaches the shell on this machine (uinput pads are
  skipped), so the pad shot used a throwaway build forcing `keys_seat = None`, not a seated pad.
- Several local seats with one on the keyboard (`--pads 0,k`) not run live; only the menu test covers
  which seat gets the keys.
- The pause Settings in a ride not shot; the ride fills `keys_seat` the same way, untested live.

## Next

none
