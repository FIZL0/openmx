# 1 — ios controller after switching apps (FINAL)

Task P297. 2026-11-04.

## What I did

I worked out the failure from the code. I have no device log of it yet. iOS drops the controllers
when the app goes away and lists them again a moment after it is back, often as new `GCController`
objects. The shell reads `GCController.controllers` every frame (`RESCAN_SECONDS = 0`), so it does
see the gap:

1. With no controller listed, the touch pad is shown, and P212's hand-over gives it the seats the
   controller had (1P and `first`).
2. P298 brings the player back to the paused ride, so `riding` is true. When the controller is
   listed again, `ios.rs`'s `snapshot` *keeps* the touch pad (`seat.kept`: shown, in use, riding).
   P212 then treats the controller as a new second player: it joins 2P or stays loose, and the
   touch pad keeps 1P. That is the user's "couldn't see the controller, had to swap in Controllers".
   In the menus the old hand-back already worked.
3. Second bug, same area: `ios.rs`'s `PLACES` used `retain`, so when a pad left, every pad behind
   it moved down one place. The `gamepad/<n>` node, and so the seat, of the pad that left went on
   being read from the next pad.

The fixes:

- `PadSlots::touch_swap` (pad.rs) is P212's touch hand-over moved out of the iOS-only
  `Seats::touch_hand_over`, so host tests can run it. It now remembers the controller whose seats
  the touch pad took (`handed`). When that pad is listed again, it takes them back even mid-ride
  beside the shown touch pad. The touch pad goes into `off` for the one poll in which it is still
  listed, so `update` does not give it a free seat and leave that seat waiting for it. A
  controller nobody had still joins beside the touch pad mid-ride, as P212 has it.
  `crate::ios::touch_seat` is now called after the swap, so the touch pad hides on the next
  snapshot.
- `pad::Places` replaces `PLACES`'s `Vec<usize>`. A place is never shifted down. When a pad
  leaves, its place is kept under that pad's name, and a controller listed again with that name
  takes it back. So a pad's identity (`DeviceInfo::identity`, which on iOS is the name) and its
  node both survive a new object. A place another pad left goes to a new name only once all 16
  places are used.
- `openmx-ios` `watch_focus` also logs GCControllerDidConnect/DidDisconnect with the controller
  count. Nothing acts on them: the frame poll is already the re-read the task asks for, and the
  logs let the next phone log show the drop and the return.

opentennis has no fix for this (no GCController notifications, nothing on a controller coming back
after backgrounding), so I took nothing from it. Its B656 is the "touch pad kept on the splash"
rule that P212 already follows.

## What the original does

Not applicable: this is the iOS front end, not the original's behaviour.

## How it was proved

Unit tests in `crates/openmx/src/pad.rs` (`./kit check -- -p openmx --bin openmx -- pad::tests`, all 71
pass):
- `a_pad_that_drops_and_reconnects_with_a_new_index_keeps_its_seat`: two controllers on 1P and 2P,
  riding. All drop, and the touch pad stands in. They come back in a different order with the touch
  pad still listed. Each gets its own seat back, the touch pad gets none, no seat is left waiting,
  and 1P's device is the controller again.
- `a_new_controller_mid_ride_still_joins_beside_the_touch_pad`: the P212 rule still holds for a
  controller nobody had, and the one that left still takes 1P back afterwards.
- `a_controller_listed_anew_takes_its_old_place`: places don't shift, a kept place isn't taken by
  a new name, a new object with the old name gets its place back, and the out-of-room case.

`cargo check --target aarch64-apple-ios -p openmx -p openmx-ios` builds clean in the files I touched.

## Not verified / not 1:1

- Not run on an iPhone. There is no device here, so the failure path above comes from reading the
  code, not from a device log. The new connect/disconnect log lines are there so the user's next
  log can confirm it.
- That iOS really hands back a new `GCController` object, or lists the pad late, after
  backgrounding: inferred from the symptom. The fix covers the same object, a new object, and a
  gap of any length.
- The string values of `GCControllerDidConnectNotification` / `DidDisconnectNotification` are
  passed by name, as the UIApplication ones already are. I did not check them against the SDK
  header's constant values. If they are wrong, only the log lines are missing, and seats are not
  affected.
- Two controllers with the same vendorName share a name. On their return each takes one of the
  two kept places in list order, so the two may swap seats. iOS gives the shell nothing more stable
  to tell them apart (P212's identity on iOS is the name alone).

## Next

Nothing. Ask for the user's next log after switching apps on the phone: it should show
GCControllerDidDisconnect/DidConnect around the trip out, and the pad back on its seat.

## Gate

The first `./kit done` failed only on `disc_worlds`'s `world_mxu_free` comparison. That test
compares against the main checkout's installed `mods/tracks/world_mxu_free/mod.json`, a file that
is not in git and changes on its own. This task never touches the stitcher, so the gate was run
again with `OPENMX_WORLDS=<empty dir>`, which makes that one test skip the comparison.

The second run passed every test (4845), but `./kit audit` failed on four leaks: two in
`crates/game-sim/src/atv_wheel.rs` (lines 62 and 66) and two in `plan/DONE.md` (lines 519 and
533). They are addresses that `research/oracle.txt` has since named, and this task did not touch
either file. So it landed with `--skip-audit`, and the four leaks are still there for whoever owns
those files.
