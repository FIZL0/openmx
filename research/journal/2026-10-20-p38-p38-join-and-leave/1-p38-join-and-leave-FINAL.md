# 1 — p38 join and leave (FINAL)

Task P38. 2026-10-20.

## What I did

- **Checked the prerequisites first** (per the slot-check habit): `plan/TODO.md` has P35, P36 and V22 open.
  `../s5` is on `task/P35` with ~1200 uncommitted lines in `crates/openmx/src/pad.rs` (`Seat`, `SEATS = 8`,
  `PadSlots<P>` with `held`/`last`/`waiting`/`forget` — a seat that waits for its own pad). `../s1` is on
  `task/P36` with an untracked `crates/openmx/src/menu.rs` and `crates/openmx/tests/menu.rs` and `mod menu;`
  in `main.rs`. Writing either file here would be an add/add conflict with P36, and editing `pad.rs` would
  collide with P35's rewrite.
- **New `crates/openmx/src/join.rs`**: the join screen as pure state over a device id `D` (a pad's identity
  or the keyboard — whatever P35's seats end up keyed by), std and `game_sim` only, so a test includes it
  with `#[path]` the way `tests/ride.rs` includes `pad.rs`.
  - `Seat<D>`: `Open`, `Taken(d)`, `Gone(d)` (its controller unplugged; the seat is kept for it).
  - `Join::press(d, Press)`: unseated Cross joins the first open seat; seated Left/Right moves to the nearest
    open seat that way; seated Circle leaves; seated Square lets go of every gone seat; seated Start is
    `Outcome::Done`, unseated Circle `Outcome::Back`.
  - `Join::update(present)`: a taken seat whose controller is no longer present goes `Gone`; a gone seat
    whose controller is back is `Taken` again (and its Cross, if it beats the update, still goes home).
  - `Join::views()`: the seats in play (taken or gone), in seat order — what the split screen draws, one
    view each, open seats skipped.
  - `Join::with(seats, count, Opened::Pause)`: the screen over a ride's seats, for the pause menu.
  - `presses(before, now)`: the presses between two of the game's button words (`Control::mask` bits).
  - `ON_SCREEN = 4` today, `MOST = 8` (P35's `SEATS`, V23's eight).
- **New `crates/openmx/tests/join.rs`**, four tests, every press through a `Report`'s button word and
  `presses`, so a held button is one press: join, move (past nobody, never swapping), leave, a full screen
  refusing a fifth, Back/Done; eight seats on an eight-seat screen; and across an unplug opened from the
  pause menu — gone not open, still a view, nobody joins or moves into it, the same pad back in its own
  seat, Square letting it go.

**Taken from opentennis** (`crates/hst/src/play/main_menu/assign.rs`, `Menu`, and
`crates/hst-play/src/play/controls.rs`'s `PadSlots`): Cross to join a free seat and Circle to leave, the
`Seat` vocabulary, and the ←→ move. **Changed**: every controller drives itself (no 1P picking up and
carrying the others' devices); ←→ moves into the nearest *open* seat instead of swapping, because here the
other seat is someone's rider; a dropped pad's seat is kept and shown gone, instead of opentennis freeing
it (P35's `PadSlots` makes the same change mid-ride); Square lets gone seats go; no computer seats (the
front end has no AI riders on this screen).

## What the original does

Not read: the brief is a front end addition ("Menus and front-end: yours to design"), and R9 found the
original's two-player game binds human i to pad i at spawn with no join screen of its own.

## How it was proved

`./kit check -- -p openmx --test join`: `TESTS PASSED: 4 tests`. Pure state, driven by generated button
words; nothing of the original to compare against.

## Not verified / not 1:1

- **Not on screen.** Nothing draws the join screen and nothing opens it: P36's front end (its screens, its
  pause menu, its look from the disc's menu art) is not merged, and its own `menu.rs` is where the screen
  belongs. Unblock: P36 lands, then wire `Join` in as a screen of its menu and from its pause.
- **Not over P35's seats.** `Join` keeps its own `Seat` list; it is not yet fed P35's `PadSlots` (its
  `present` pads, its keyboard) and does not write the seats back as `--pads`/`PadSlots::with_seats`.
  P35's `PadSlots::waiting`/`forget` and `Join`'s `Gone`/Square are the same rule twice until then; the
  wiring should make `Join::update` read from `PadSlots` (or replace `Join`'s copy with it). Unblock: P35
  lands.
- **The split screen does not follow the count yet.** `Join::views()` is the list it should follow; V22
  (the split itself) is not done. Unblock: V22.
- **Joining or leaving mid-ride** (from the pause): the screen's rules are the same, but spawning a joined
  rider and removing a left one is the ride's job and needs P35's multi-rider shell. Untested.
- **Pad presses come from the game's button word**, not from P35's per-device js events — the mapping of a
  host pad's buttons to Cross/Circle/Square/Start/d-pad is `pad.rs`'s `BUTTONS`, untouched here. The d-pad
  only: a stick flick does not move a seat (opentennis's arrows are the same).
- **The keyboard as a controller**: `D` covers it, but which keys are Cross/Circle on this screen is
  P36's bindings' call.
- Not run in the shell at all, so not seen by eye.

## Next

When P35 and P36 are merged: in P36's `menu.rs`, add the join screen as a `Screen` reached from the main
menu and from the pause menu, opened with `Join::with` over P35's seats; each frame feed it
`PadSlots`' present pads (`update`) and each pad's `presses` (keys through P36's bindings); on `Done`
write `riders()` back as the seats; draw a card per seat with `label`, `ON_SCREEN` of them; hand
`views()` to V22's split. Then move `tests/join.rs`'s cases into the brief's `tests/menu.rs` if P36's
file wants them there.
