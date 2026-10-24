# 1 — per player pause (FINAL)

Task P84. 2026-10-23.

## What I did

**The menu: one pause a seat.**
- `Menu` stays one state machine with one `stack`. It now also holds a `Side` for each extra seat:
  that seat's stack, cursors, tab, pauser, switch pick, leaving confirm and typing.
- `Menu::with_seat(seat, f)` swaps that seat's `Side` into the menu's own fields, runs `f`, and swaps
  it back. Every screen, row and confirm works unchanged inside a seat's pause.
- `Menu::view(seat)` is a clone with the seat swapped in, for drawing and for tests.
- `Menu::paused(seat)` says whether a seat has a pause open.
- `riding()` is true only when every seat's stack is empty.
- `pause_for(seat)` opens that seat's own pause. `driver()` is now always 0, because each pause hears
  its own seat's pad.
- `step_seats` steps 1P's own menu, then every other seat inside `with_seat`.
  - If an act leaves the ride (Restart, Main Menu, Quit, Online), every pause is closed and that act wins.
  - If two seats act in the same frame, the extra acts wait in `later` and come out one a frame. The
    shell takes one act a frame.
- `main()` and `splash()` close every seat's pause.
- `lost()` (a pad gone) no longer takes 1P's pad off a split while somebody's pause is open.

**The shell (main.rs).**
- `Seat.paused` follows `menu.paused(i)` each frame.
- A paused seat gets `riding = false`. Its pad is still polled, but its machine is not stepped, so the
  rider holds still in place with its speed kept. Resume steps on from that exact state.
- Escape (the keyboard) opens 1P's pause, or presses Back in it.
- The ride's modifiers are applied when any seat's pause closes.
- A paused seat's bike, and its flown plane, are drawn at `vehicle::PAUSED_ALPHA`, which is P57's
  put-back ghost alpha (`Ghost::START`, 0.5). The helper is `vehicle::shown_alpha`.
  - The rider's `Ghost` state is **not** touched. Touching it would start the 3 s fade and change
    online hashes in ways the pause has no reason to.
  - Resume is solid at once.
- Menu drawing:
  - GPU: each paused seat's menu is drawn as sprites sized to its view rect, then moved to the rect's
    corner (`Menu::pause_views`).
  - Software: each paused seat's menu is drawn into that view's frame before `split::put`.
  - The whole-frame menu draw happens only when the screen is not split.
- Seat offers (P57's put-back marker offer) are not affected. A paused seat already returns none.

**Online.**
- `vehicle::Wanted` (the shell byte at `net::SWITCH`) has a new `paused` bit, bit 5. A paused seat's pad
  goes out at rest with that bit set, so the pause and the resume are inputs on the agreed tick, like
  N17's machine switch.
- `net::rides::Rides::step` reads the bit for every seat. A paused seat is not stepped, and it is left
  out of `rider_contact::among`, so nobody hits it and it hits nobody.
- `Online::paused(d)` reads the newest logged pad's bit, so every machine draws that seat see-through.
  The local seat's own menu is local, like the machine-switch menu.

**Race modes.** The shell has no race mode today, so every ride is a freeride. Every ride now pauses
per seat. Nothing was kept back for races.

## What the original does

The original has one pause for the whole screen. Its split screen stops everyone. This task is the
port's own behaviour, asked for in the brief. No retail function was read or matched.

## How it was proved

- `crates/openmx/tests/menu.rs` `in_a_split_2p_pauses_alone_and_1p_rides_on_tick_for_tick`, over 200
  ticks:
  - Two riders on the stand-in bike with RIDE_LIMITS, both on the gas. Each tick is the shell's
    `Seat::pass` (pad polled, `pass_marked` only when not paused).
  - 2P presses Start at tick 60, Down at 80, and Start at 120.
  - 1P's `Sim::hash` equals a lone 1P's on every tick.
  - While paused:
    - Only `view(1)` has the Pause screen. 1P's `screen()` is none.
    - `pause_views` over Stacked's rects is only `(1, rects[1])`.
    - 2P's Down moves only 2P's cursor.
    - 2P's position and speed stay equal to their values at the pause.
    - `shown_alpha` is `PAUSED_ALPHA`.
  - 2P's second Start gives `Act::Resume`, and 2P rides on more than a foot from where it stopped.
- `crates/openmx/tests/netplay.rs` `one_seat_pauses_alone_online_and_both_peers_agree`:
  - Host and g over 480 rounds, with 10% loss and a 3-round lag. Rollbacks happened.
  - The host's seat is paused for ticks 150..330 of its clock.
  - There are no desyncs, and both peers agree on the pads and hashes of more than 400 settled ticks.
  - Replaying the logged pads offline through `Rides` gives the logged hash before every tick.
  - The paused seat's `Sim::hash` does not change on any of the 180 paused ticks (151..=330, one tick of
    input delay later). g's hash changes on every tick.
  - The host's seat rides on for more than 50 ticks after the resume.
- `netplay.rs` `a_paused_seat_is_passed_through_online` (needs the disc; it ran here — checked with a
  probe panic, since removed):
  - The two disc-collision riders from game-sim's `rider_contact.rs` stand 1.5 ft apart.
  - Unpaused, they push apart: seat 1's chassis differs from the same run without collision files.
  - With seat 0 paused, seat 1's chassis is exactly the one from the run without files, and seat 0 does
    not move.
- `a_wanted_machine_goes_in_one_byte` now round-trips the `paused` bit too.
- Updated menu tests that assumed the shared pause: `picking_the_biplane_in_the_pause_puts_that_player_in_it`,
  `start_held_for_many_frames...` and `over_a_split_only_the_pausers_pad_answers_the_confirm`.
- `./kit check --all` is the gate (run by `./kit done`).

## Not verified / not 1:1

- **The shell was not run with a real two-player split pause.** No second pad or keyboard reaches a
  window here. The drawing paths have not been seen on screen: the half-view menu on GPU and software,
  and the see-through paused bike. The menu in a view is the full menu laid out at the view's size, so
  how it looks in a 640×224 half is unchecked.
- **Offline "passed through" is trivially true.** Local split riders are separate `Sim`s with no
  rider-vs-rider contact at all, paused or not. Only online (each seat its own Sim with
  `rider_contact::among`) is there any contact to leave out, and that case is tested.
- **The paused seat's pad is still polled offline.** This matches the old `riding = false` freeze. The
  input manager's held/edge state keeps up while paused, so the first tick after a resume sees the pad
  as it is. Not compared with any other choice.
- **Online, the paused seat's pad is also held at rest.** `Rides` skips the seat anyway, so the pad
  bytes are unused while paused.
- **Resume is solid at once.** There is no fade back in, unlike P57's put-back ghost, which fades over
  3 s. The brief says "until they resume", and I read that as solid on resume.
- **Acts from two seats in the same frame are queued**, one a frame. A leaving act wins over all others
  and closes every pause.
- **The keyboard always drives 1P's pause**, as before.
- **Race modes** do not exist in the shell, so "races may still pause everything" was not built or
  tested.
- **The online see-through alpha comes from the newest logged pad** (`net.log.last()`), not from the
  drawn tick's own pad. Under a rollback it can be a tick or two ahead of the state drawn.

## Next

Nothing for this task. A human with two pads could look at a split pause on screen: the half-view menu
layout and the ghosted bike.
