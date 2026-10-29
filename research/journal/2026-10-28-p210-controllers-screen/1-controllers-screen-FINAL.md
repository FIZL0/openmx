# 1 — controllers screen (FINAL)

Task P210. 2026-10-28.

## What I did

**(1) The Controllers screen** (`Screen::Assign`, `--screen controllers`). It is a row on the main menu
(after Settings) and on the pause (after Settings, opened from 1P's pause only). It is modelled on
opentennis's `crates/hst/src/play/main_menu/assign.rs`:

- A "No seat" column on the left holds every device on no seat, in plug order. Beside it are the
  seats 1P..8P, four to a line, each holding the card of the device that drives it. A card shows
  the seat number, the device's name, and arrows for the ways it can move. A device whose pad is
  out shows "Gone: plug it back in" and keeps its seat, as `PadSlots` keeps it.
- Every device works its own card with its own presses (`Menu::step_assign`, the per-device state
  in `join::Assign`):
  - Left/Right step one column. Column 0 is no seat. Moving into a column with a device in it
    swaps the two.
  - Cross lights the card (press-to-find: a 30-tick glow and a "This is you" line). From no seat it
    also joins the first open seat.
  - Square takes the device off its seat.
  - Circle/Start from a host (1P's device, the keyboard, or whatever now sits on 1P) closes the
    screen. Closing is refused with a note while 1P is empty.
- opentennis's fixes, carried over:
  - **A device moved onto 1P takes the screen at its first press there.** Until it presses, 1P's
    old device still drives. On that press `Menu::p1` becomes it, and the shell skips its usual
    `Seats::set_first`, which would have wiped the seats. That call is now made only when Seats'
    first device differs.
  - **The joining card flashes.** A Cross from no seat joins and lights.
  - **A 0.3 s slide.** `SLIDE_TICKS = 18` at the menu's 60 Hz, eased out. Both cards of a swap
    slide.
  - **Press-to-find over a ride.** The pause's Controllers screen hears every device: each rider's
    pad seat, the pads in no seat (the new `Seats::loose`), and the keyboard on its own. This goes
    through `Readers` per device in the ride loop. 1P's joint press is not stepped twice.
  - **The pause's slots are drawn.** The screen is drawn in stage ems like every other screen, so
    it fits any aspect (checked by shots at 16:9 and 5:4) and any split view the pause is drawn
    in.
- Every change is written at once to `pad.rs`'s seats (`PadSlots::assign`), which `Seats::frame`
  then hands out:
  - each pad goes on its seat, held if it is plugged in and waiting if not;
  - the keyboard goes on its seat, or rides no seat (the P46 behaviour beside a pad 1P) if it was
    put on none;
  - pads put on no seat are kept off until unplugged;
  - 1P's device becomes `first`;
  - every seat with a device in it is *placed*.
- Online, the screen offers only this machine's riders' seats (`open = riders`).

**(2) Taking over.** This lives in `Seats`/`PadSlots`, so the menus and a ride behave the same.

- A pad's press takes 1P when 1P is the keyboard or the touch pad (`PadSlots::take_first`).
  - It refuses a touch pad, and under `--pads`.
  - It refuses a pad that is a person's: one on 1P, one on a seat placed on the Controllers screen,
    or one on a seat a rider reads (`Seats::set_riding`, which the ride loop feeds from `pad_of`).
  - The keyboard stays on 1P beside the pad (`keys_at = Some(0)`), so it keeps steering. A free
    pad's old auto seat is let go.
- A keyboard press takes 1P back (`take_back`) from a 1P pad that has been idle for `IDLE_US`
  (5 s). It does not do this for a pad placed on 1P, nor while the keyboard rides a seat of its own.
- The gate is `Seats::set_take_over(Menu::takes_over())`:
  - **on** in a ride, and on the menus once there is a 1P;
  - **off** on the splash, the join and split screens, the lobby, the garage, the on-screen
    keyboard, the Controllers screen and over the pause. On the pause, P76 says a free pad's
    Cross joins the ride as a new player, and that should win there.
- The touch pad: web/touch.js already hides it when a real pad connects. `Menu::lost` no longer
  sends the menus back to the splash when the lost 1P pad is the touch pad (id contains
  "Touch controls"). The keyboard works the menus until the real pad's first press takes 1P.
- The shell prints "1P: <name> takes over (the keyboard rides on beside it)" and "1P: the keyboard
  takes 1P back from an idle pad". After a takeover it moves `Menu::p1` (`took_over` in main.rs).

**Tests** (tests/menu.rs, run as `./kit check -- -p openmx --test menu`: 190 passed). They use two
fake Switch Pro controllers through the real `Seats` (FIFOS lock), the keyboard, and the Menu read
the way main.rs's front end reads it:

- `the_controllers_screen_moves_and_swaps_the_seats_the_next_ride_reads`: pad 2 Left → 2P (swaps
  with pad 1), keyboard Right → swaps with pad 2 onto 2P, then pad 2's press on 1P makes it the
  menu's 1P and lights its card. The next ride's `Seats::frame` gives 1P pad 2's Cross, 2P the
  keyboard's Left and 3P pad 1's Cross, and the seats hold over further frames.
- `a_pad_pressed_while_the_keyboard_holds_1p_takes_1p_and_the_keyboard_still_steers`: pad 1 taken
  off on the screen, then its Cross on the main menu takes 1P (the menus' p1 too). The ride's 1P
  report carries the keyboard's Left, and the pad's Cross, beside it.
- `a_2p_pad_never_takes_1p`:
  - a pad placed on 2P on the screen presses and nothing moves;
  - a pad on a rider's 2P seat in a ride presses and nothing moves;
  - while the free 3P pad does take 1P.
- `the_keyboard_takes_1p_back_and_a_real_pad_takes_it_from_the_touch_pad`: `PadSlots` take-back,
  the refusal for a placed pad, the touch pad's takeover after it hid, and `Menu::lost` keeping 1P.
- The one existing test changed: `the_walk_a_rider_takes_…` counted seven Downs to the pause's
  Main Menu. It is eight now, under the new row.

## What the original does

Nothing: the original game has one pad per port and no assign screen. This is the port's own front
end, after opentennis.

## How it was proved

- The four tests above, on the real `Seats` over two FIFO fake pads.
- `--screen controllers --shot` at 1920x1080 and 1280x1024, software renderer: it looked right
  (keyboard on 1P, seven dashed open seats, the No seat column, the hints).

## Not verified / not 1:1

- **No mouse drag to swap.** The brief lists opentennis's drag fix. openmx's menus have no pointer
  except the pause music player's clicks (P174), and building drag-and-drop for one screen was more
  than this task. Follow-up P212.
- **No prompt.** opentennis's takeover asks first (takeover.rs). The brief says a pad pressed
  *becomes* 1P, so here it happens at once, with a printed line and no on-screen popup.
- **The idle threshold (5 s)** for the keyboard taking 1P back is my choice; nothing gave a number.
  It is tested only on `PadSlots` (`take_back`), not with a real clock through `Seats::hands`.
- **Keyboard presses beside a 1P pad are the pad's on the ride-time Controllers screen.** Seat 0's
  report merges the two, so the keyboard's own card there is moved only when it rides no seat or a
  seat of its own.
- **The keyboard's card on "No seat"** after a takeover: it rides beside the 1P pad, but the screen
  can only show one card on 1P, so it shows the keyboard on no seat. Leaving it there on the screen
  keeps the P46 behaviour (the menus and 1P's pause, Cross to join), not "beside 1P".
- **Only 1P's pause opens the Controllers screen.** Another rider's row says so in a note. The
  screen moves every rider's device, and the other pauses are their own (P84).
- **A free pad's press in a ride with the keyboard on 1P takes 1P rather than joining (P76).** By
  design, from the brief. The pad joins as 2P from the pause's Cross (the gate is off there), the
  Local Game join, or this screen.
- **Not run on a real pad or in the browser.** The touch pad path is checked by `PadSlots` and
  `Menu::lost` unit cases only, and the pause-over-split drawing was not shot.
- The ride-loop wiring (per-device readers, Escape as the keyboard's Back on the screen) is
  compiled and covered by the gate's shell tests only, not by a test of its own.

## Next

Nothing: landed with `--skip-audit`, since `./kit audit`'s 7 leaks are all other tasks' addresses in
plan/DONE.md (biplane_tire_* and radar_update), none in this diff.
