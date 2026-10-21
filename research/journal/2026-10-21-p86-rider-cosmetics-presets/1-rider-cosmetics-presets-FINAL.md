# 1 — rider cosmetics presets (FINAL)

Task P86. 2026-10-21.

## What I did

- **The wardrobe off the disc** (`roster.rs`, "the look"): `appstart.res`'s `rider\gear.dat` lists
  every jersey-and-pants pair (`[JerseyPants]`: its JerseyMaterial, PantMaterial, NameFont,
  NumberFont) and the pieces (`[Helmets]`, `[Goggles]`, `[Gloves]`, `[Boots]`); `bike\bikes.dat` gives
  each bike's `NumberPlateFont`. `Wardrobe::read` keeps only the pieces `Rider\custom.res` has paint
  for, so every item the garage offers can be drawn. A `Look` is outfit + four pieces + name +
  number; `Wardrobe::materials` turns it into renames of the shared body's base materials
  (`THQ_Helmet_01` → `Fox_Helmet_02` …), applied after the gear set's own `.sub` renames.
- **The garage** grew the original custom-rider screen's rows below Gear Set
  (`ui\singleplayerbikeriderselect.lyt`: Gear Set, Jersey Pants, Name, Number, Helmet, Goggle,
  Glove, Boot), plus a Preset row. Gear Set is the existing Rider + Colours rows. Each seat's
  column has its own keys for typing a name or number (Cross types, Square deletes, Start keeps,
  Circle drops), with the front end's keyboard letters and the number pad of
  `mxfrontendnumentry.lyr`. The column scrolls (seven rows shown) and the keys replace the rows
  below the one being typed.
- **Presets**: Cross on Preset saves the seat's whole pick (bike, set, gear set, look) under
  "Name Number" in `riders.txt` beside `settings.txt` (atomic write); left/right on Preset loads
  each saved one in turn; `Garage::load_preset` loads one by its key. `garage.txt` gains
  `seatN.look` and `seatN.preset` lines (written only when not default, so older files and builds
  read the same).
- **Drawing it** (`dress.rs`): a per-rider `NameDefault` texture painted as the original paints
  its copy (below), registered with the new `Assets::make_texture` before each rider's body and
  bike load, so the jersey back and both number plates of that rider's models pick up their own.
  `custom.res` is added to the archives when anyone is dressed. Other local seats' copies of the
  shared bike parts are loaded under per-seat names so their plates differ.
- **HUD**: `overlay::Art::tags`: the view's own `#72 Kaito` under the score, other riders' numbers
  (or the first letter of a name) beside their radar marks.
- **Online**: `openmx_net::Pick` gained `look` (`#[serde(default)]`, so an older peer's message
  still reads); the first seat's look goes in the lobby pick; the lobby seat list shows each seat's
  `#number name`.

## What the original does

Boot program, research only (`./kit fn <addr>` to read):

- `./kit fn 0x0020b240` loads `BikeText.res`'s fonts `name1..6`, `number1..6`, `plate1..6` and the
  `*3_black` ones, and the `NameDefault0..9` textures. Strings from 0x42e3e8 on (`SglRacUI.res`,
  `BikeText.res`, `NameDefault%d`, `name%d`, `number%d`, `plate%d`, `%d`, `%s`).
- `./kit fn 0x0020b860` walks the riders; `./kit fn 0x0020baa0` copies `Rider\Tex\NameDefault` to
  that rider's `NameDefault%d` and draws on it with the text-onto-texture routine at 0x001af2b0
  (font, texture, text, x, y, 1, 1): the name (`%s`) in the jersey's name font centred at (32, 15),
  the number (`%s` of the profile's number, a 5-byte buffer; AI riders `%d`) in the number font at
  (32, 39), and the number in the bike's plate font at (31, 79) and (31, 109).
- The font names come from the `NameFont` / `NumberFont` / `NumberPlateFont` strings at 0x440d88,
  read by `./kit fn 0x003716c0` (the gear/bike data reader). Defaults when absent: name1, number1,
  plate3.
- The disc's own `NameDefault` (64x128) has `Baumsteiger` and `000` baked in; the plastics and
  number-plate parts (`250sx_plastics.psg`, `250sx_numberplate.psg`) and the body all map it.

## How it was proved

- `crates/openmx/tests/menu.rs`
  `a_seat_dresses_its_rider_saves_the_look_and_a_second_seat_rides_it_after_a_restart`, on the disc:
  by pad reports only, seat 1 changes colours, jersey, helmet, goggles and boots, types `Kaito` and
  `720` (Square deletes the 0, Done closes), saves the preset; seat 2 on its own pad loads it from
  the Preset row; `ride_picks` gives both the same look; `dress::Kit` gives both the same body
  repaint and the same painted texture; `garage.txt` + `riders.txt` written and read by a fresh
  Menu give the same picks and presets; a third garage loads it by key. Checked it really ran on
  the disc (a deliberate panic printed `thq1, THQ_Helmet_01, THQ_Goggles_02, , Generic_Boots_02, 72, Kaito`).
- `crates/openmx/tests/overlay.rs` `the_riders_names_and_numbers_are_drawn`.
- roster unit tests: the wardrobe read from the gear/bikes files, a look and preset reading back as
  written, a seat typing its name and number and saving/loading.
- Looked right: the painted texture for `Kaito`/`72` beside the disc's stock `NameDefault` — same
  places, same faces; the garage with the keys open, two seats (rendered from a throwaway test,
  not committed).
- `./kit check`: TESTS PASSED (full run).

## Not verified / not 1:1

- **Not seen in a ride.** No frame of a dressed rider or bike was looked at — only the menu tests,
  the texture, and the garage screen. The bike plates and jersey back taking the per-rider texture
  is by the mapping the parts already have. (P88 filed: hold it beside the original.)
- **Online rides draw only the local rider**, so peers' cosmetics are seen in the lobby seat list
  and in the radar tag, not on the track. Filed as N14.
- **Long names**: squeezed across to fit 62 px. How the original fits a long name is not known.
- **Number length**: kept to 3 digits. The original's buffer is 5 bytes (so up to 4 digits);
  its number pad's limit was not read.
- **Pro riders' gear sets** that have no `[JerseyPants]` entry use the default fonts name1/number1
  — the defaults from the reader, not checked against a pro rider in the original.
- **Plate colours**: the bike's plate font (`plate3` white or `plate3_black`) is drawn as the data
  says; the disc's stock texture has black digits with a white edge, which neither font reproduces
  exactly — it is a baked picture, not the original's per-rider output, so this was not compared.
- **The garage is the port's layout**, not the original's custom-rider screen: rows in a column per
  seat, typing in the column instead of the original's full-screen text entry, and Preset is the
  port's own (the original saves a look to the memory-card profile).
- **Picking an explicit piece the gear set already wears** (e.g. `THQ_Helmet_01` over a THQ set) is
  allowed and is the same as Own.
- **A seat's empty name and number keep the disc's texture** (`Baumsteiger` / `000`), as the
  original's AI riders would not; untested which the original shows for a nameless profile.
- The up-arrow over a scrolled column sits on the "Player N" heading; cosmetic.

## Next

Nothing for this task. N14 (peers ridden in their looks online) and P88 (beside the original).

(Landed with `--skip-audit`: the audit's six leaks are other tasks' names in plan/DONE.md, none of
this task's files.)
