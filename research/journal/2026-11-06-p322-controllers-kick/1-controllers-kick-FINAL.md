# 1 — controllers kick (FINAL)

Task P322. 2026-11-06.

## What I did

- `join::Assign` keeps `let_go`: the seats a device was moved off onto no seat (Square / Take Off, or a
  card dragged to No seat — both go through `move_to(d, 0)`). `take_let_go()` hands them over, leaving
  out any seat that has a device on it again.
- `Menu::kick()`: while the Controllers screen is over a ride (the stack holds the pause), each seat
  let go is read as a pad seat on `ride_devices()`. That player goes out: `Act::Leave(i)` on a local ride
  (P76's path: rider removed, `reseated`, split laid out for the new count), `Act::LeaveSession(i)` online
  (N11's path, `Online::leave_session`, which leaves at a tick both machines agree on). One a call, so a
  second one is read on the players as the first leave left them. The Controllers screen stays up.
- Not kicked: 1P (rider 0), and the last player. Decision: the Controllers screen already refuses to
  close without a controller on 1P ("1P needs a controller."), so 1P's rider waits for one; and online a
  `LeaveSession(0)` takes every seat of the machine, which is the pause row's job, not a card move.
  This is stricter than the brief's "1P cannot be emptied while it is the only seat": with 2P riding,
  emptying 1P does not kick 1P either. Kicking 1P with 2P riding would leave pad seat 0 empty with the
  remaining rider on pad seat 1 and the screen unable to close, so this was left out on purpose.
- Off a ride (the main menu's Controllers) nothing is kicked; the list is cleared.
- main.rs: after the step_idle loop, a `Stay` frame takes `front.menu.kick()` as its act, so the
  existing `Act::Leave` / `Act::LeaveSession` handling does the rest.
- Only a move to no seat kicks. A rider's pad moved to another seat (an empty 3P, say) leaves its
  rider's pad seat empty but does not kick; it is a move, not a removal, and the brief names removal.
- opentennis: its assign screen has no in-ride kick; nothing taken.

## What the original does

Nothing: front end, ours to design.

## How it was proved

`crates/openmx/tests/menu.rs` `a_pad_taken_off_on_the_controllers_screen_kicks_its_player_out_of_the_ride`:
a Free Ride, 2P drops in (DropIn rig, two views), 1P's pause → Controllers. 1P's pad off: nobody leaves.
2P's pad off and back on before the ride reads it: nobody leaves. 2P's pad off: `Act::Leave(1)`, one
rider, one Full view, screen still up, once only; closed and resumed, 1P rides on 10 more ticks. Online
(`session`, 2 riders): 1P off → no kick; 2P off → `LeaveSession(1)`, once. Main menu's screen: no kick.

## Not verified / not 1:1

- The online half is proven at the menu only (the act is `LeaveSession(1)`); that a local player 2's
  `leave_session(1)` leaves at an agreed tick with checksums agreeing is lobby.rs's N20 test
  (`a_second_player_on_an_online_machine_sits_down_and_leaves_again`), not re-run through a shell with the
  Controllers screen up. A two-shell test driving the screen was not written: the shells' Controllers
  screen takes pad devices the online-script harness does not plug.
- main.rs's wiring (the kick taken on a `Stay` frame) was not run in a live shell; the test rig mirrors
  it line for line.
- 1P is never kicked by this, even with others riding (above): a choice, stricter than the brief.
- A local ride whose seats are not open (a tape, `seats_open` false) ignores the `Act::Leave`, as the
  pause's Leave Session row is absent there too: that rider rides on with no controller.

## Next

Nothing.
