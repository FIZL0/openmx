# 1 — keyboard joins a pad-1P ride (FINAL)

Task P173. 2026-10-28.

## What I did

- Cause (P171's finding): with a pad as 1P and no `--pads`, `PadSlots::keys` gives the keyboard no
  seat (it only works 1P's pause, P46), and the ride's drop-in reads idle *pad seats* only, so the
  keyboard had nothing to join on.
- `PadSlots` keeps `keys_joined`, the seat the keyboard took by joining. `keys_free_seat(riding)` is
  the first seat after 1P's that no pad holds or waits for and no player reads, only while the
  keyboard drives none and 1P is a pad (not under `--pads`, not before the splash, not with the
  keyboard 1P). `join_keys` / `leave_keys` set and clear it; `keys()` returns it first; `update`
  gives no new pad that seat; `set_first` (the splash's next 1P) clears it. `Seats` wraps the three.
- main.rs's ride loop: a `keys_reader` reads `keyboard_alone()` each frame; while a free seat exists
  and 1P's pause is not open (the keyboard works that pause, so its Cross is 1P's OK there) its
  presses go into `idle_presses` as that seat's, so `Menu::step_idle` joins it exactly as an idle
  pad's Cross does. On `Change::Join(d)` with `d` the keyboard's seat, `join_keys(d)`; on any
  `Change::Leave(i)`, `leave_keys(devices[i])`. The join cue ignores the keyboard (it is always
  there, so counting it would show the cue in every pad-1P ride with a free seat).
- Escape: once the keyboard rides its own seat (menu `keys_seat` > 0), Escape opens and backs out of
  that player's pause instead of 1P's.
- Nothing from opentennis: its keyboard is a seat from the start (seats.rs), it has no pad-1P
  keyboard drop-in to copy.

## What the original does

Not applicable: no drop-in in the original; a shell feature.

## How it was proved

- `pad.rs` `the_keyboard_joins_a_pad_1p_ride_on_a_free_seat` (PadSlots: free seat skips held and
  ridden seats, keyboard keeps its seat across a new pad and an unplugged 1P, leave frees it for the
  next pad, splash resets it, no free seat with keyboard 1P / `--pads` / no splash).
- `pad.rs` `a_joined_keyboard_drives_its_seat` (Seats over a fifo pad: before, the keyboard is
  alone and seat 1 empty; joined, seat 1 plays the keyboard's keys and 1P's pad has none of them;
  left, alone again).

## Not verified / not 1:1

- The main.rs wiring (keyboard Cross → `step_idle` → `Change::Join` → `join_keys`, Leave → `leave_keys`,
  the Escape routing) was not run live: no keystroke reaches a window on this desktop (wtype types
  into the focused window and nothing can focus ours), and the shell has no fake-keyboard input. It
  is covered only through the pad.rs tests of the pieces it calls and the existing menu tests of
  `step_idle`.
- Not checked with a real pad as 1P and a real keyboard by eye: no pad here.
- The keyboard cannot join while 1P's pause is open (its Cross is that pause's OK, P46); a choice,
  not a bug.
- Under `--pads` the keyboard still has no way to drop in (it has a seat only if `--pads` names one);
  left as `--pads` sets it.
