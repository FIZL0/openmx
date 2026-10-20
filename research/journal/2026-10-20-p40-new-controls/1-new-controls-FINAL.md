# 1 — new controls (FINAL)

Task P40. 2026-10-20.

## What I did

- `crates/openmx/src/pad.rs`: `Scheme { New, Original }`, `New` the default, one per seat in
  `Wiring::schemes` (so the existing `set_wiring` path from the front end carries it). `Scheme::ride`
  is the remap: a pure function of the 32-byte report, applied in `main.rs` where a seat's report
  is handed to the engine's pad port. Under `New` R2 moves onto Cross and L2 onto Square, bit and
  pressure byte together, and the pad's own Cross/Square/L2/R2 are let go; Select passes through
  (the simulation reads nothing from it) and is the camera (`Scheme::changes_camera`). Under
  `Original` it is the identity. A missing pad (status 0xff) is never touched.
- The remap is in the ride's hand-off and not inside `Pads::build`, because the menus read the same
  seat reports and Cross stays their OK under either layout. Two things inside `build` do depend on
  the seat's layout, set by `Seats` per frame (`Pads::scheme`; a bare `Pads` is `Original`, so
  `tests/ride.rs` and the older tests read exactly what they did): the keyboard's throttle ramp also
  presses R2 under `New` (it stays on Cross too, for the menus), and `--throttle r2|l2` is ignored
  under `New`, where the triggers are already the gas and brake.
- js button 6 is now Select in the default bindings (it was Start, for pads with no `BTN_SELECT`).
  A pad whose sysfs `capabilities/key` lists `BTN_SOUTH` and not `BTN_SELECT` has its buttons from 6
  on read one up (`DeviceInfo::no_select`), so its Start is still Start. No sysfs entry: no shift.
- `controls.txt`'s header says what each layout does and that Cross and Square are kept free.
- `main.rs`: `--controls new|original|new,original,…` (one per seat, the last carried on). The flag
  took a bindings file before; a value that is not a layout list is still taken as that file, and
  `--bindings FILE` names it plainly. The ride prints `controls: …`. Help text and module doc updated.
- `menu.rs`: `Layout` (menu.rs compiles alone in `tests/menu.rs`, so it mirrors `pad::Scheme` the
  way `Gas` mirrors `Throttle`; `main.rs` asserts the seat counts agree), a `layout = …` line in the
  front end's `controls.txt`, and rows `1P controls` … `4P controls` on the Controls screen; seats
  5–8 are the file's. The throttle row reads `Original throttle` now, since it only matters there.
- opentennis has no control-scheme setting to take from (grepped `crates/*/src`); the per-seat list
  syntax follows this repo's own `--pads`.

## What the original does

Nothing new: the original's binding is what `game_sim::ride::Controls::from_pad` reads (Cross gas,
Square brake, R2 front brake, L2 clutch, L1+R1 reset) and the shell's L1 camera walk. The new layout
is a shell remap in front of it, so no simulation code changed.

## How it was proved

pad.rs's own tests (`./kit check` passes, whole workspace):
- `under_the_new_layout_the_triggers_are_the_gas_and_the_brake`: R2 at seven travels, through the
  port's poll and `Controls::from_pad`, is exactly the gas the original reads off Cross at the same
  pressure; L2 the same against Square's brake; front brake and clutch zero; Cross and Square held
  do nothing under `New` and are full gas and brake under `Original`.
- `select_cycles_the_camera_under_the_new_layout`: Select's press edges cycle, L1 does not; under
  `Original` L1 does, Select does not, and L1 after R1 (the reset chord) does not.
- `the_original_layout_maps_exactly_as_before`: 20,000 random reports through `Original.ride` are
  unchanged, a missing pad is unchanged under `New`, an idle report is idle under `New`, and a
  keyboard seat on `Original` (throttle on a trigger) builds the same report a bare `Pads` did.
- `the_keyboard_and_the_wiring_under_the_new_layout`, `a_layout_list_reads_and_says_why_not`,
  `a_pad_with_no_select_keeps_its_start`; `tests/menu.rs`'s file round trip now holds a mixed layout.
- Ran the shell: `--headless --frames 3` with no flag, `--controls original`, `--controls new,original`
  and `--controls bogus.txt` print the expected `controls:` line; 9 seats is refused. A `--menu
  --screen controls --shot` frame shows the rows laid out.

## Not verified / not 1:1

- No physical pad in the session: Select on a real pad's js button 6, the `no_select` shift on a
  real device, and R2/L2 travel on real triggers were proved only through the tests' fake devices
  and axis values. The capability-bitmap format was checked against this machine's
  `/sys/class/input/event*/device/capabilities/key`; no js device was plugged in to compare.
- Under the new layout the original's front brake (R2) and clutch (L2) have no control at all. The
  brief keeps Cross and Square free, so nothing was put on them; a rider on `New` cannot clutch or
  front-brake until a later task binds them.
- An existing `context/controls.txt` bindings file written before P40 still binds `PadBack` to
  `start`, so on that file Select on a pad is Start and the new camera is only on Tab. Only a freshly
  written file has `select = Tab PadBack`. Not migrated, to keep the player's file theirs.
- The camera under a played tape follows the session's `--controls` layout, not the one the tape
  was recorded on (a tape doesn't store it). Presentation only; the simulation replays the same.
- The front end's Controls screen has rows for 1P–4P only; seats 5–8 are set in `controls.txt`.
- Not driven in a ride through the window by hand (no keystrokes reach windows on this desktop);
  the in-ride camera and gas under `New` are covered by the unit tests, not by a live ride.

## Next

Nothing for P40. A later task could put the front brake and clutch somewhere on the new layout.
