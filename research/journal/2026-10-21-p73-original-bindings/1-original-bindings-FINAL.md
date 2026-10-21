# 1 — original bindings (FINAL)

Task P73. 2026-10-21.

## What I did

**Read the original's binding table, whole, and the one routine that builds it.** The original never
asks a pad for a button: the bike, the biplane, the cameras and every front-end screen ask for a
*command* (0 to 0x4d), and the input manager's table says which of the sixteen controls that command is.
The full table, its layout, its builder, the four presets, every caller and what each command's readers
do with it are in `research/oracle.txt` under "P73" (all in `#` comments, so no audit names).

**Ported it.** `crates/game-sim/src/bindings.rs` (new) is the table: `Command` (named for every command
whose readers were read), `PRESET_COMMANDS` / `PRESET_CONTROLS` (the four presets' eight commands),
`FIXED` (the 52 bindings every preset shares, in the builder's call order), and `Table` with `build`
(the builder, as the original runs it: preset on one pad, then the fixed ones on both, nothing cleared),
`SHIPPED` (preset 0 on both pads — what all three riding save states hold), and the two ways the
original reads a command: `value` (the first binding's strength) and `pressed` (a press on either
binding), plus `held`.

**Every ride command goes through it.** `ride::Controls::from_pad` is now `Controls::through(pad,
&Table::SHIPPED)`: gas, brake, clutch, the three trick buttons, the d-pad and the L1+R1 reset chord are
read off their commands. What the table does not have is gone: **R2 is no longer a front brake, L3 is no
longer balance, and there is no reverse button** — `front_brake`, `rear_brake`, `reverse` and `balance`
stay at rest from a pad (the fields remain, for the sim's own uses and tests). The biplane's
`PlaneInput::from_pad` reads its eight commands through the same table.

**The shell follows the table.** In `crates/openmx/src/pad.rs`:

- the camera button is the original's: **Select** (command 0x21) changes camera when neither look back
  (L2) nor the other camera (R2) is held — under both layouts. It was L1 under Original before (P50);
  L1 is the clutch and half the reset chord in the original, never a camera.
- `Scheme::commands()` lists what each layout binds: Original is the shipped table's ride commands
  exactly; New moves gas to R2 and brake to L2 (user-asked, P40) and keeps the L1 clutch (P67) and
  Select camera; it binds nothing the original has not got.
- the Controls screen's keyboard list (`keyboard_layout`) now names exactly the original's commands
  (Gas, Brake, Clutch, Trick 1–3, the sticks, D-pad, Camera, Reset, Pause) and nothing else.

P72 (reverse on L3) is *not* done here — the brief puts it after this task.

## What the original does

- **The table.** `*(*(app_pointer)+0x104)+0x648`, 0x750 bytes, every slot -1 when allocated. Rows: a
  keyboard-like device's, pad 0 first, pad 1 first, pad 0 second, pad 1 second, one more kind. Each row
  is 0x4e four-byte control indices in `input::Control` order. In all three states: both pads
  identical, preset 0, no second binding, the two other rows empty.
- **One builder** (`./kit fn 0x00200f00`, args app, pad, preset): eight commands by preset on that pad
  — look back, other camera, gas, brake, clutch, trick 1–3 — then 52 fixed commands on both pads.

  | preset | look back | other camera | gas | brake | clutch | trick 1 | trick 2 | trick 3 |
  |---|---|---|---|---|---|---|---|---|
  | 0 (shipped) | L2 | R2 | Cross | Square | L1 | Circle | Triangle | R1 |
  | 1 | L2 | R2 | Square | Cross | L1 | Circle | Triangle | R1 |
  | 2 | L2 | R1 | Cross | Square | R2 | Circle | Triangle | L1 |
  | 3 | L1 | R1 | R2 | Square | L2 | Circle | Triangle | Cross |

  The fixed 52 (d-pad 00–03, Start pause 04, the biplane's rudder L1/R1 14/15, the camera's L3/R1
  zoom/wide modifiers 1e/1f, Select camera 21, the free camera's, the front end's menu commands, the
  L1+R1 reset chord 4c/4d, …) are listed in oracle.txt and `bindings::FIXED`.
- **When it changes.** By player preset (the profile, set on the Controller Presets screen, which
  rebuilds) and by vehicle (a vehicle switch builds the profile's preset for the bike and preset 0 for
  the five other kinds) — never by game mode. The Map Stunts screen does not write the table.
- **The bike's levers are switches.** Gas, brake and clutch are 1.0 when their command's value is not 0;
  the pressure is dropped. (P9's "Cross is analogue" holds for the biplane, whose throttle reads the
  pressure; it does not for the bike.) Gas and brake held together below speed 5 is the original's only
  reverse. There is no front-brake command, no reverse button and no balance button.
- **The camera.** L2 pressed: look behind until released. R2 pressed while moving: the other camera
  until released. Select pressed with neither held: the next follow-camera preset.

## How it was proved

- `crates/game-sim/tests/bindings.rs`, through `./kit check`:
  - `the_shipped_table_is_the_save_states` — the whole table (both pads, both bindings, the two unused
    rows) read out of **3** riding save states' RAM (first_recording, recording2, biplane) equals
    `Table::SHIPPED` slot for slot. Proved it ran by turning its last line into a panic once: "3 save
    states" (then restored).
  - `the_builder_is_the_originals` — the original's own builder run in the EE interpreter over a table
    reset to -1, for **2 pads × 4 presets × 3 states = 24** runs; each equals `Table::UNBOUND.build(pad,
    preset)`, and the builder writes nothing outside the table and its stack frame. Proved it ran the
    same way: "matched 24 times".
  - `every_command_reads_its_binding_and_an_unbound_one_reads_nothing` — each of the 16 controls alone,
    on both pads, against all 0x4e commands; the 18 unbound commands read nothing with all 16 held.
  - `the_ride_reads_each_command_off_the_control_the_table_names` — each control alone moves exactly
    its commands' `Controls` fields; L2, R2, Select, Start, L3, R3 move none; an unbound table moves
    nothing.
  - `the_planes_commands_are_the_tables` — the biplane's eight.
- `crates/openmx/src/pad.rs` tests: `select_cycles_the_camera_under_either_layout`,
  `each_layout_binds_the_originals_commands_and_no_other`, `the_controls_screen_lists_the_keyboard`, and
  the trigger-throttle test (a plain R2 now moves nothing in a ride).
- The recorded replays did not move: both human recordings press only 0/255 pressures, so a lever read
  as a strength is already 1.0 or 0 there, and neither recording rides L2/R2/L3 (their tests held under
  `./kit check`).
- **The synthetic platforms vectors did move, as they should.** `tests/platforms.rs`' canonical run
  feeds LCG-random buttons, which press R2 (was front brake) and L3 (was balance); they now do nothing.
  Regenerated with `print_the_vectors` / `print_the_riders_vectors` and spliced const by const:
  `HASHES`, `TRACE_DIGEST`, `POS_Y`, `VEL_Y`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`,
  `RIDERS_HASHES`, `RIDERS_DIGEST`, `RIDERS_WIRE_DIGEST`. `FINAL_DUMP_LEAVES` and `EVERY_LEAF_HASH` did
  not move (same state type); `HUNDRED_TICKS` did not either. Not re-run on another architecture.
- `./kit check -- --no-fail-fast`: everything passed but netplay's `four_peers_and_eight_riders_on_a_bad_network`
  ("only 248 ticks settled", the known load flake); `-p openmx --test netplay` alone: TESTS PASSED.

## Not verified / not 1:1

- **The bike's levers are still strengths, not switches.** The original sets gas/brake/clutch to 1.0
  for any non-zero value; the port still passes the pressure through. Identical for every recording
  here (0/255 only), different for a feathered DualShock 2 or the New layout's analogue triggers.
  Not changed because New's analogue R2 gas is user-asked (P40) and making it a switch is the user's
  call; filed as a plan item.
- **Gas + brake reverse, `StoppieBrake`, and the bike's own input limiter are not ported.** Read
  (oracle.txt), not implemented: they are sim behaviour beyond the binding table, and reverse is P72's.
- **Commands 05–0d, 23, 25, 26 have no reader found** (the pad's query slots take the command in a
  register, so a constant search misses them). They are bound as the original binds them, unnamed.
- **Commands 16–1d belong to two vehicle classes not identified here**, and 27–30 to the engine's free
  camera; named by reader only, not ported.
- **The "other camera" (+0x64) is not identified**, and the camera handler's two "not held" flags are
  approximated by the shell as "L2 / R2 not held".
- **Look back, the other camera, and the right stick's zoom/wide modifiers are not in the shell.** Their
  buttons are bound (the table), nothing reads them; so `Command::RIDE` leaves them out of the Controls
  screen. Under New, L2/R2 are gas/brake, so those two camera commands have no button — a user question,
  filed.
- **Presets 1–3 are in `bindings` but not offered in the menu.**
- **The keyboard-like and kind-1 rows are not modelled** (empty in every state). Pads past the second
  read the second pad's row (the original has two).
- **Command names are the project's.** The preset screen's labels are in `ui.loc`, whose values are
  encoded and were not decoded.
- **The new platforms vectors are this machine's only** (no cross-architecture run here).
- **No live emulator check.** Everything is from three save states' RAM and the interpreter running the
  original's builder, not PCSX2 driven over PINE.

## Next

Follow-ups filed: P77 (lever switches + reverse, user question), P78 (camera buttons under New, user question), P79 (presets 1–3 in the menu).
