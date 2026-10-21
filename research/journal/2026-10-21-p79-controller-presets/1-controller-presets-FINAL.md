# 1 — controller presets (FINAL)

Task P79. 2026-10-21.

## What I did

- **The simulation reads its own table.** `Sim` has a `bindings: Table` (shipped by default),
  `Sim::set_presets([pad0, pad1])` builds it with P73's `Table::built`, and `Sim::table()` is what
  `poll`/`poll_all` read through: `bindings` on the bike, `Table::SHIPPED` with a plane up. It is a
  constant of the ride like `phy`: cloned with a snapshot, kept local by `snapshot::decode`, and not
  in the hash, so every existing hash and platform vector is unchanged (preset 0 is `SHIPPED`).
- **Why not a report remap like the New scheme.** Presets 2 and 3 cannot be expressed as one:
  under preset 2 L1 is the third trick *and* half the reset chord, but under the shipped table the
  third trick is R1, which is the chord's other half. No report gives "trick 3 held, reset-right not".
  So the preset has to be in the table the ride reads, as in the original.
- **Menu.** `controls.txt` has a `preset = N` line (one number, or one per seat like `layout`);
  `Controls::presets`. The seat's Controls row now steps through ten choices: New with the two
  biplanes, then Original on presets 0–3, each with the two biplanes. Preset 0 is labelled
  `Original` as before; the others `Original preset N`. The keyboard list beside it is per
  (layout, preset) — `Menu::keyboard` has five lists, `menu::keyboard_at` indexes them — and names
  each preset's commands on their keys (`Bindings::keyboard_layout_on`, `Scheme::commands_on`).
- **Shell.** `--preset 0..3|0,2,…` (overrides the file like `--controls`). Each frame, a seat's sim is
  set to its preset (`Seats::preset`: the pick under Original, 0 under New — the New remap is written
  against the shipped table). The camera's hold-back test reads the seat's table on the bike
  (`Scheme::changes_camera_on`), the plane's as before. The debug overlay's gas/brake/clutch markers
  read the seat's table (`Hud::bindings`). Startup prints `controller preset: …` when any is not 0.
- **opentennis**: has graphics presets only, no controller presets; nothing taken.

## What the original does

P73's journal has it (`2026-10-21-p73-original-bindings`): one builder, four presets moving eight
commands (look back, other camera, gas, brake, clutch, trick 1–3), 52 fixed bindings on both pads;
the profile's preset is built for the bike, preset 0 for any other vehicle; the Controller Presets
screen rebuilds on leaving.

## How it was proved

- The builder itself was proved by P73 (interpreter, 2 pads × 4 presets × 3 states).
- `crates/game-sim/tests/presets.rs`: for each preset and each of the 16 controls held alone,
  `Sim::poll` and `Sim::poll_all` give exactly `Controls::through(Table::built([p, 0]))` (pad 1 on
  its own preset-0 row); preset 0 equals `Controls::from_pad`; preset 1 gas on Square, preset 2
  clutch on R2 and trick 3 on L1, preset 3 gas R2 / clutch L2 / trick 3 Cross; L1→R1 reset chord
  and the d-pad on every preset; a plane reads `SHIPPED`; snapshot/resync/hash behaviour.
- `crates/openmx/tests/menu.rs::the_controls_row_offers_the_originals_presets`: the ten-step cycle,
  wrap both ways, 2P untouched, labels, the file round trip and parse of bad values; the existing
  save/load test now carries a preset.
- `pad.rs`: `the_controls_screen_lists_each_presets_commands`,
  `each_preset_holds_the_camera_back_on_its_own_buttons`.
- Shell, end to end: a hand-written 120-tick tape holding Square on free06,
  `--play … --preset 0` ends at 0.0 mph (Square is the brake), `--preset 1` reaches 34.2 mph at 3 s
  and ends about 65 units further along z (Square is the gas).

## Not verified / not 1:1

- **Not checked against the original's Controller Presets screen.** No live run of the original with
  presets 1–3 picked; the ride's reading of them rests on P73's builder proof plus the port's own
  table lookup. Why: the screen is a front-end path that needs pad navigation through the profile
  menus, and the ride-side behaviour is already a function of the proved table.
- **The labels are the project's** ("Original preset 1"…): the original's names for its presets
  are in `ui.loc`, which is still not decoded (as P73 noted). The menu also counts from 0, as the
  table does, which the original's screen may not.
- **A tape does not carry the preset.** `PadTape` holds reports, and under the Original layout a
  report is read through the preset's table, so a tape recorded on preset N replays the same only
  with `--play … --preset N` (the shell takes the tape's seat's preset from `--preset` alone, not from
  `controls.txt`). Under the New layout, preset 0 always, so those tapes are unaffected.
- **Online play rides preset 0** for every seat: the preset is not in the handshake, so a seat's pick
  is dropped (and said so on the console) rather than desyncing the peers. Follow-up N12 sends it.
- **The original rebuilds the table on leaving its presets screen; the port sets it every frame**
  from the seat's current pick. Equivalent within a ride (the pick only changes from the menus).
- **Players past pad 1 in one `Sim`** (fixtures only) read pad 1's row as before; the shell gives
  each seat its own `Sim` with its preset on pad 0, so every shell seat gets its own preset.

## Next

Nothing for this task.
