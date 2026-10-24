# 1 — per seat presets (FINAL)

Task P143. 2026-10-23.

## What I did

- A modifier preset is now one player's set (`menu::preset_of`): the Player row's seat's per-seat
  knobs written onto every row of the kept `Mods`, plus the world knobs (`Knob::per_seat` false).
  Keeping it alike on every row means `Mods::text` writes it as plain `word=pct` pairs, so the
  settings.txt `preset = name | …` line is the one set with no `.N` player numbers, and the type of
  `Settings::presets` did not change (no other code reads it).
- Save Preset (`Menu::save_preset`) takes the Player row's seat; Load Preset (`Key::LoadPreset`,
  menu_pages.rs) goes through `menu::load_preset_onto`: the set's per-seat knobs into the Player
  row's seat, the world knobs into the world's row, every other seat untouched. Any seat loads any
  preset.
- A preset line from before reads its seat 0 as the set (`Settings::parse` runs `preset_of(.., 0)`),
  so the user's saved presets load onto 1P exactly as before; it is rewritten as the one set on
  the next save of settings.txt.
- The Save Preset row's tip says it is this player's set, loadable by any player.
- Nothing taken from opentennis: it has no modifier presets.

## What the original does

Nothing: Game Modifiers and their presets are the port's own (P55), a front-end addition.

## How it was proved

`crates/openmx/tests/menu.rs::a_modifier_preset_is_one_players_set`, through the menu's pad
input: 1P saves Fast at 150% top speed / 90% gravity, changes both, 2P loads it (2P 150, 1P keeps
140, 3P 100, gravity back to 90); 2P saves Slip, 1P loads it; the file's preset line has no player
numbers and parses back equal; an old whole-table line (`bike_speed.1=150 bike_speed.2=300 grip.2=50`)
reads as its seat 0 and loads onto 1P as before; under the pause, 2P loads Fast, the pause closes and
the ride's set (`settings.modifiers`, what the shell's remodify takes) has 2P at 150 and 1P at 100.
The P55 test's preset assertion now checks for 2P's set.

## Not verified / not 1:1

- The pause-close case is checked at the menu: the settings' modifiers after Resume and
  `take_changed`. That the shell then hands them to `Mods::resim` is P98's path, unchanged here and
  covered by tests/remodify.rs; not re-run through the shell with a loaded preset.
- An old preset's per-seat values for seats other than 1P are dropped when it is read (the task's
  rule: its seat 0 is the set). The line is rewritten without them at the next settings save.

## Next

Nothing.
