# 1 — track pick full name (FINAL)

Task P252. 2026-11-02.

## What I did

- `menu::title_and_text`: a name's title before its first colon and its text after it; no split
  where either side would be empty.
- `menu::marquee(s, fits, step)`: all of `s` where it fits, else the longest run of characters that
  fits from a start that holds [`MARQUEE_HOLD`] (12) steps at 0, moves a character a step till the
  rest fits, holds there and starts over. Character steps, not a smooth slide: the ink layer has no
  clip, so a run of whole characters is what fits a box.
- `menu_screens.rs` `tracks` / `cards`: the lit card's name and its line scroll, the others keep
  the ellipsis; a split name's text replaces the card's "Last ridden"/source-game line (for the
  disc worlds that line said "MX Unleashed", the title again). Under the still: the title, then
  the text at 1.5 em, each scrolling in its room.
- `Menu::look` (V30's "draw again only when it looks different"): on the pick it steps every
  `MARQUEE_FRAMES` (10) instead of every still (150) only while `track_name_scrolls` says the lit
  track has something too wide, so a short name still draws once per still. 10 divides 150, so
  the stills turn on time either way.

## What the original does

Nothing to match: the disc's pick has five towns with short names. This is the port's own menu.

## How it was proved

- tests/menu.rs `a_worlds_name_splits_into_a_title_and_its_text` (the user's own string, and every
  `DISC_WORLDS` name), `a_name_wider_than_its_box_scrolls_and_shows_every_character` (real ink
  widths: every step fits its box, moves at most a character, and over 400 steps every character
  was shown), `the_picks_look_steps_with_a_scrolling_name` (a long mod name's look changes at a
  marquee step, Dallas's does not).
- Looked at: `openmx --screen tracks` with settings.txt `track = world_mxu_nat`, on a private
  headless weston: cards read "MX Unleashed / WORLD", "MX Unleashed / the freeride maps", and the
  lit one's heading "MX Unleashed" over "every national".

## Not verified / not 1:1

- A scrolling name was not looked at on screen (no mod with a long name installed in the scratch
  run); its steps are only checked by the tests above.
- tests/menu.rs `the_pick_lists_the_disc_worlds_after_the_five_and_rides_them` fails, as on main
  before this (it expects two disc worlds; P253 added the third).

## Next

none
