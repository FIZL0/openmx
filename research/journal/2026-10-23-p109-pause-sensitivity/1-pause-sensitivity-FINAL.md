# 1 — pause sensitivity (FINAL)

Task P109. 2026-10-23.

## What I did

First I went through every suspect in the brief, reading the code:

- **Start held across the pause's first frame / a press carried in from the ride.** Not the cause. The
  pause reader (`Front::reader`) reads 1P's report every ride tick, the ride's included, so a held Start
  makes one edge and the pause takes nothing more until Start is let go. A seat > 0's Start pauses for
  that seat (P41, deliberately), and its reader has already used up the edge. Tested anyway (below).
- **The menu logic stepping twice.** Not the cause. Every screen's Up/Down moves the cursor once per
  `Press` (`Menu::step`, `walk_page`, `keyboard`). Headings are skipped on purpose, since they aren't rows.
- **Stick and d-pad both firing.** Not the cause. `Reader` ORs the stick's direction bits into the
  button word before it takes edges, so the two together are one hold.
- **Repeat firing on the first frame.** Not the cause. The held count starts at 0 on the press frame,
  so the first repeat comes 20 frames later. **But** the repeat counted frames, and the pause is stepped
  at the ride's 30 Hz ticks (`pace::TICK`) while the front end runs at 60. So the pause repeated at half
  the front end's rate, after twice the delay.

That leaves the input layer. I found three ways one push becomes two presses there. This machine has no
real pad, so I could not reproduce any of them on the user's hardware. All three are in the code, and
each one is fixed and tested:

1. **P53's press queue counted bounces and sources.** `Pads::event` queued a press for each source's
   down-edge: a hat axis and, separately, `BTN_DPAD_*` buttons. A pad that sends its d-pad both ways
   (some hid-generic / 8BitDo modes do) put *two* Downs in the queue for one push, and the menu got
   them as taps on two frames: two rows. A contact that bounces (down, up, down within a few ms) also
   queued twice, even inside one frame. Before P53 a bounce inside a frame was invisible. P53 made it
   two presses. This fits "after P53/P118, d-pad, every menu".
   Fix (`pad.rs`): a control is pressed only when it goes down across *all* its sources
   (`Pads::pressable`: bindings' buttons | hat | d-pad buttons). And only if it has been up for at least
   `BOUNCE_MS` = 35 ms, timed by the joystick events' own millisecond stamps (now read from the event,
   where before they were thrown away).
2. **A bounce straddling a frame.** Down in report N, up in N+1, down again in N+2: `Reader` saw a
   fresh edge. Fix (`menu.rs` `Reader`): a control let go for less than `BOUNCE_US` (35 ms, in the
   reader's own frame time) and down again is the press it was. A pad's deliberate quick re-press still
   gets through as a tap from (1), which is timed by the events.
3. **Two hosted devices seeing one push.** Past the splash the front end ORs the *edges* of 1P's pad
   and the keyboard (`Menu::step_devices`), and the pause did the same (`keys_reader`). A desktop that
   maps a pad's d-pad/A/B to arrow keys/Enter/Escape (Steam's desktop layout does, for one) delivers the
   push on both devices, often a frame apart: two rows, or a Start that opens the pause and closes it on
   the next tick. That matches the brief's "opens, closes or moves" exactly. Fix: `Reader::read_joint`
   reads several reports as one held state, and `Readers::read_hosted` gives the joint press to the
   hosted devices (1P + keyboard, past the splash; per device on the splash, which asks who pressed). The
   pause reads 1P's seat and the lone keyboard through one `read_joint`. `keys_reader` is gone.

Also:

- The stick now has hysteresis: it presses at 64/127 and lets go below 40, so a stick hovering on the
  line no longer presses and lets go over and over.
- The repeat is in time, not frames: `REPEAT_AFTER_US`/`REPEAT_EVERY_US` are the front end's 20 and 5
  frames at 60 Hz. `Reader::at(hz)` says the rate, and the pause's readers (`Front::reader` and
  `start_readers`) are `Reader::at(RIDE_HZ)` = 30.

The tests' tap helpers (`tap`, `tap_seat`, `tap_device`, `tap_heard`, lobby.rs's `tap`) released for
one 60 Hz frame. That is 30 presses a second, which (2) now reads as a bounce. They now rest three frames
(50 ms) between presses, a hand's pace. No test's meaning changed.

opentennis: nothing to take. Its menus sit on Bevy/gilrs input, which has no equivalent of this layer.

## What the original does

Not relevant. The menus are ours to design (AGENT.md). The simulation isn't touched: the ride's own
reports are built exactly as before. Only the queue of menu taps and the menu `Reader` changed.

## How it was proved

Unit tests, `./kit check -- -p openmx --test menu --test lobby` (157 passed):

- `pad.rs` `one_d_pad_push_is_one_press_from_both_sources_and_through_a_bounce`: a fake Pro
  Controller with the hat *and* d-pad buttons through the real fifo+sysfs path. Hat + button for one push:
  one tap. A 3 ms bounce inside a frame and across one: no extra tap. A re-press 50 ms later: a tap.
- `tests/menu.rs` `one_d_pad_push_moves_exactly_one_row_on_every_screen_and_tab`: Main, Free Ride,
  every Settings and Garage tab (P118's L1/R1), Mods, Online, Modifiers, the keyboard, the pause.
  Down and Up held 1–19 frames, plus two bounce shapes, must each equal exactly one step's state.
- `one_push_of_a_pro_controller_s_hat_moves_one_row_through_the_pad_layer`: the same through
  `Seats::devices` with the user's pad model: tap inside a frame, held six frames, bounce in a frame,
  bounce across a frame. One row each.
- `start_held_for_many_frames_opens_the_pause_once_and_leaves_it_open`: 90 ticks of Start at 30 Hz.
  Pause open, cursor 0, until a fresh Start resumes. The same for seat 2 over a split, and 1P's Start
  does not close seat 2's pause.
- `a_stick_resting_just_off_centre_does_not_move_the_cursor`: ±20…60 for 600 frames on every screen
  moves nothing. A stick wobbling on the line (58–70) is one row.
- `a_held_push_repeats_at_the_front_ends_rate`: 30 Hz and 60 Hz readers move the same rows over one
  and two seconds (9 and 21), with nothing before a third of a second.
- `a_push_both_the_pad_and_the_keyboard_report_is_one_press`: pad Down then keyboard Down a frame
  later through `read_hosted`: one row. An unhosted pad's hold doesn't swallow 1P's press. The pause's
  joint read: pad Start, then the keyboard's a tick later, leaves it open.

## Not verified / not 1:1

- **Which of the three causes the user actually hits is not known.** No pad exists on this machine
  (P53's note), and the user's Pro Controller is on another machine. All three mechanisms are in the
  code and each is fixed. A cause outside these three would still show, and the splash's
  "Controllers:" / "Not read:" lines are where to start.
- **Whether the user's desktop maps the pad to keys (Steam's desktop layout or similar)**: not checked.
  The fix covers it either way.
- **The 35 ms bounce window is a judgment, not a measurement.** No real d-pad's bounce was recorded. A
  hand's fastest double tap is up for longer, but a deliberate re-press on the *keyboard* that is up for
  under 35 ms (one pause tick) is now dropped. Pads keep theirs through the event-timed taps.
- **joydev event times are assumed to be milliseconds that only wrap at u32** (`jiffies_to_msecs`).
  The comparison wraps (`wrapping_sub`), but it was not read against a live device.
- **The ride's pause reader assumes 30 Hz.** A ride that falls behind and steps several ticks a frame
  still counts each as a tick, which is what the ride does too.
- The shell (main.rs) wiring of `read_hosted` / `read_joint` is covered only through the same methods in
  tests, not by running the shell with two real devices.

## Next

Nothing for this task. If the user still sees a double move, ask which pad and whether Steam (or any
mapper) is running, and look at the splash's device lines.
