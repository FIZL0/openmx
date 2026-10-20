# 1 — p35 seats and pads (PART)

Task P35. 2026-10-20.

## What I did

The shell has seats: up to eight (`pad::SEATS`), each the keyboard, a pad or nobody, with pads joining
and dropping mid-session. All in `crates/openmx/src/pad.rs` (tests/ride.rs compiles that file in by
`#[path]`, so nothing new could live in a second module it would have to pull in too).

- **Taken from opentennis, as it is there:** `Seat` (`parse`, `flag`) and `readable`'s Steam filter with
  `ignore_list` from `crates/hst-base/src/seats.rs`; `PadSlots` (`update`, `keys` with its B388
  stand-in) and `Bindings` (`parse`, `text`, the `controls.txt` format, unknown names reported and
  skipped, a line with nothing understood keeps its default) from `crates/hst-play/src/play/controls.rs`.
  Its `seated_pads` and `steam_ignore_list` tests came along, rewritten for eight seats.
- **New:** `Seat::parse_list` (`--pads`, at most eight, one keyboard), `Seats` — the hot-plug manager
  over `/dev/input/js*` — and `SeatInput` (`Empty`, `Playing(report)`, `Waiting`).
- `Pads` is now one seat's state: a device, the keyboard, or both. `Pads::open` went (`Seats` opens
  devices), `Pads::report` takes the key state as a function rather than the window, and `build` reads
  `Bindings` instead of `PAD_BUTTONS`/the old `keyboard()` fn/`KEY_STICKS`' keys, which are now the
  default bindings (`KEYBOARD` is the old table as data). The default binding is exactly the old one,
  checked by `bindings_round_trip_and_parse`, and every older pad.rs test and tests/ride.rs's flip on
  the keys passes unchanged.
- main.rs: `--pads k,0,1,-`, `--controls FILE` (default `context/controls.txt`, written with the
  defaults on first run, beside the disc as opentennis puts it), seats opened at startup, and each
  frame `Seats::frame` → seat `i` feeds engine port `i` (`PADS` is 2). While 1P's seat is `Waiting`
  `engine.frozen` is set: the engine skips the pass (no tape frame either, so a recording has no hole),
  and the body, the chassis, the overlay and the per-second line read the freeze (`moving`).

### What changed for a raw js device instead of gilrs

- **No connect/disconnect events.** gilrs reports them; a js device does not. A new pad is a `jsN`
  node that was not there before, so `Seats` re-reads the directory every 0.5 s (`RESCAN_SECONDS`); an
  unplugged pad is a read that fails (ENODEV) or reads end-of-file, and `Pads::drain` now closes the
  device and marks it `gone` instead of reading a dead file for ever.
- **No entity, so an identity.** gilrs gives an `Entity` per connection; `PadSlots` here is generic
  over what names a pad, and the shell uses `DeviceInfo::identity`: vendor:product, `uniq`, `phys` and
  name from `/sys/class/input/jsN/device`. The `jsN` number is not it — the kernel hands the number to
  whichever pad comes next. Two identical pads that also share `phys` get `#2`, `#3`.
- **A seat waits for its own pad.** opentennis frees an unplugged pad's slot and the next pad plugged in
  takes it. Here a seat remembers its last pad (`PadSlots::last`), is `waiting` while it is out, takes
  it back first when it returns, and a pad nobody has seen joins the first seat nobody waits in;
  `forget` lets a seat go. A rider mid-ride is not a tennis slot between points.
- **The keyboard can take a waiting seat over.** Where the keyboard drives the waiting seat (1P without
  `--pads`, or opentennis's B388 stand-in), Start on its keys forgets the pad and rides on; the same
  press is not passed on as the game's Start, which would freeze the ride it resumed.
- **Buttons are numbers.** gilrs names its buttons; a js device's are indices in the kernel's gamepad
  order, so `controls.txt` names that order (`PadSouth`, `PadEast`, `PadWest`, `PadNorth`, `PadL1`,
  `PadR1`, `PadBack`, `PadStart`, `PadMode`, `PadL3`, `PadR3`) and `Pad0`..`Pad11` reach any of them.
  Keys are named by X keycode (`KEY_NAMES`) instead of Bevy's `KeyCode`. The lines are the game's
  sixteen controls plus the four right-stick keys, not tennis's actions; the d-pad hat, the sticks and
  the triggers are axes on a js device and stay fixed, as opentennis's sticks do.
- **Dropped:** opentennis's browser touch-pad case in `update_with`, `ignore_input` (no `--shot`-only
  input gate here — the shell already read js0 in an unattended run before this task) and the
  rebinding screen's helpers (`rebind_key`, `names`, …), which are a menu's (P36/P38).

## What the original does

Nothing to port: seats and host pads are the shell's. R9 (how the original hands pads to players) has
not landed; nothing here depends on it, because every seat produces the same DualShock 2 report.

## How it was proved

Unit tests in `crates/openmx/src/pad.rs` (`./kit check`: TESTS PASSED):

- `seats_are_taken_in_join_order`, `a_seat_is_kept_across_an_unplug_and_replug`, `seats_set_up_by_pads`
  — `PadSlots` alone.
- `pads_join_drop_and_come_back_to_their_seats` — `Seats` end to end over fifos standing in for js nodes
  and a fake sysfs: two pads join 1P and 2P, each drives only its own seat, 1P's is pulled out (1P
  `Waiting`, its port `Report::missing()`, 2P rides on), a third pad joins 3P and not 1P's seat, the first
  comes back **as js3** and is 1P again, then the keyboard's Start takes 1P over without passing the
  press on.
- `the_keyboard_is_a_seat` — no pad: the keyboard is 1P; `--pads 0,k`: a pad 1P and the keyboard 2P, each
  with only its own hands, and 2P unaffected when 1P's pad goes.
- `bindings_round_trip_and_parse`, `a_rebound_button_drives_its_new_control`,
  `a_controls_file_is_written_once_and_read_after`, `a_pads_list_reads_back`, `steam_ignore_list`.

A 3-second unattended run of the shell on :0 wrote `context/controls.txt` and printed
`1P: the keyboard`.

## Not verified / not 1:1

- **Seats past 1P do not ride.** S54 (N players) is running in s4 and not merged: seat 1 and 2 feed the
  engine's two ports, and the ride reads port 0 only; seats 3–8 are built and reported but go nowhere.
  Handing seat `i` to S54's player `i` is the part left.
- **A pulled pad freezes the whole frame, not its player.** With one rider that is the same thing; with
  S54's players it must pause only that player, which needs S54's per-player step.
- **No real pad was plugged or pulled.** This machine has no js device and `/dev/uinput` is root-only
  (memory: no pad), so the hot-plug is proved over fifos. That the kernel answers ENODEV (and not
  something `drain` would read as "nothing waiting") on a real unplug, and that `uniq`/`phys` stay the same
  across a replug of a real USB or Bluetooth pad, is from the kernel's documented behaviour, not seen. A
  USB pad moved to a different port has a different `phys` and comes back as a new player.
- **Steam's filter was not seen against Steam.** Only its parse and the 0x28de/0x11ff rule are tested.
- **`controls.txt`'s location** is `context/controls.txt` relative to the working directory (the kit's
  root in practice), not truly beside the disc image: `Disc` does not say where the image is and
  game-render was not this task's to change. `--controls` puts it anywhere.
- **Bindings do not reach `--trace`/tapes:** a tape holds the reports, so it plays the same whatever the
  file says; that is intended, written so nobody wonders.

## Next

After S54 lands: hand `Seats::frame`'s seat `i` report to player `i` (main.rs's `world.reports`), and
pause only the player whose seat is `Waiting` instead of setting `engine.frozen`. P38 builds the join
screen over `PadSlots` (`forget`, `waiting`, `keys`).
