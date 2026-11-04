# 1 — drag a card (FINAL)

Task P214. 2026-11-04.

## What I took from opentennis

From B597 (`crates/hst/src/play/main_menu/assign_drag.rs` and `pointer.rs`, commit 9379795a):

- A press on a card is not a drag until the pointer moves a few units. Until then the card stays put,
  and a plain click is unchanged.
- The lifted card follows the pointer, drawn over everything else.
- The drop target rings.
- Letting go there makes the same trade the pads' own moves make.

What I changed:

- openmx's screen has a No seat column, so a card dropped there takes its device off.
- openmx's rules are P210's `join::Assign`, not tennis's: no computer players and no Keep/Undo popup.
  The new `Assign::may_drop` / `drop_on` reuse `move_to` and `free_to`.

## What I did

- **Windows** (`window.rs`, `gpuwin.rs`, `screen.rs`): beside P174's `clicks()`, a new `mouse()`
  returns the left button's Down / Move-while-held / Up, in order, in window pixels.
  - X11 also selects ButtonRelease and Button1Motion.
  - winit reads `MouseInput` Released and `CursorMoved` while the button is held.
  - The browser (`web_host.rs`, `web_gpu.rs`) and the iPhone (`ios_gpu.rs`) return none.
- **Menu** (`menu_drag.rs`, new): `Menu::mouse(Mouse, at, size)` reads in the view's pixels and maps
  into stage ems.
  - Down on a plugged-in card holds it (`card_drag`).
  - Moving 0.6 em lifts it (with the Move cue).
  - Up over a column calls `Assign::drop_on`: Accept cue if it moved, Refused if not. Up anywhere else,
    or before the card lifted, moves nothing.
  - Leaving the screen lets go of the card.
  - `drop_column()` names the column that would take the card; `card_point` / `assign_point` are for
    the tests.
- **Layout** (`menu_screens.rs`): the screen's boxes are now one `assign_spots`, which both the drawing
  and the hit tests read.
  - The dragged card is drawn last, offset by the drag.
  - The drop column gets a peach ring.
- **Shell** (`main.rs`, `drag_cards`):
  - On the front end, the screen covers the whole frame.
  - Over a ride, it is drawn in 1P's pause view (the only pause that opens it).
  - Window pixels go through the new `cursor::onto_frame`. This is `to_frame` without its bounds check,
    so a card dragged past the picture's edge still has a place, and dropping it there changes nothing.
  - `take_controllers` then hands the moved seats to `pad.rs` as before.

## How it was proved

- `./kit check -- -p openmx --test menu`: 261 passed. Two new tests:
  - `a_card_dragged_onto_another_seat_moves_or_swaps_it`, on P210's two-FIFO-pad rig:
    - a wiggle under 0.6 em moves nothing;
    - 3P's pad dropped on 1P swaps with the keyboard (1P rings first), `Seats` has it at once, and the
      swapped card slides;
    - a pad dropped on No seat is taken off, and dropped back onto an empty 2P;
    - dropping off the frame, past its corner, or on its own seat moves nothing;
    - pressing an empty seat holds nothing;
    - the next ride's `Seats::frame` reads 1P from pad 2, 2P from pad 1 and 3P from the keyboard.
  - `a_dragged_card_keeps_the_pads_rules`:
    - an unplugged pad's card can't be lifted;
    - its kept seat takes nobody;
    - an empty seat does take a card;
    - online with two riders, 3P is refused and 2P is taken;
    - closing the screen mid-drag lets go of the card.
- `window.rs` unit test `the_left_button_down_its_drag_and_up_are_mouse_events_in_order`, through
  `./kit check -- -p openmx --bin openmx -- window::`: 11 passed. The `cursor` test crate: 120 passed.
- A throwaway render, not committed: `Art::parts` on the screen mid-drag at 1280x720 and 1000x800.
  The Keyboard card was lifted over the Pro Controller's 2P, 2P was ringed peach, and the No seat
  column was intact.

## Not verified / not 1:1

- **No drag in the browser or on the iPhone.** The page posts only pointer-downs (`web/worker.js`
  `clicks`), and the phone posts only taps. Both now return no mouse events. Follow-up F38.
- **Not run in a live window.** The X11 and winit event reading is tested only by the unit test (X11)
  and by compiling (winit). Nobody dragged with a real mouse, because this desktop takes no input from
  me.
- **Over a split pause**, the screen's view is 1P's pause view, as for P174's clicks. This was not shot.
- **The 0.6 em threshold** is my choice. opentennis uses 6 px at 448 high, which is about 0.6 of
  openmx's ems at 45 high.
- **A plain click on a card does nothing**, as before. It does not light the card, unlike Cross.
- **A drop onto 1P does not make the dropped device the menus' 1P.** As with a pad's own move, that
  happens at its first press there (P210).
