# 1 — delete preset asks first (FINAL)

Task P246. 2026-11-04.

## What I did

- Delete Preset on Game Modifiers and on the Camera page (a player's own preset) no longer forgets on
  one Cross. It raises the yes/no P229's Save Over uses, with the cursor on Cancel: "Delete <name>?" with
  the rows Delete / Cancel. `Menu::over` gets two new kinds, `Over::DeleteMods(i)` and
  `Over::DeleteCamera(pick)`. `overwriting()` now gives back each kind's own question and rows, and
  `answer_over` runs the new `delete_preset` / `delete_camera_preset` on the yes. The forgetting itself
  works as it did before (Load Preset stays at that place in the list, clamped; the Camera page goes
  back to All Presets), and it now sets `changed` there.
- Camera's "Same as All Presets" (which shared the old Delete arm) still acts on one Cross. It only
  undoes a seat's edit to a disc preset, so it was left as it was.
- The drawing (popup and hints), mouse clicks blocked under the question, and the all-rows menu
  test's "Circle puts the question away" path all go through `overwriting()`, so they cover the new
  question as they are.
- The gate: `disc_worlds`' cold-build budgets (world_mxu_nat, world_mxu: 480 s) red two gates
  running (519-636 s at load ~30-57), as did its installed ride (71 s against 60, K24). None of this
  is menu code. I put `-p openmx --test disc_worlds` in `gate.toml [alone]`, as P294 did for
  world_ultra_load. It is a stopgap; K24 stays open for the real fix. `--skip-audit`: the audit's
  leaks on main are K26's, not this branch's.
- opentennis has no Delete Preset question to take from. The menus are ours to design (AGENT.md).

## What the original does

Not applicable: this is a front-end addition of the port's own. MX Unleashed has no user presets.

## How it was proved

Two unit tests in `crates/openmx/tests/menu.rs`, `delete_preset_asks_before_forgetting_a_modifier_preset`
and `delete_preset_asks_before_forgetting_a_custom_camera`. In each one, Cancel and Circle forget
nothing and leave `changed` clear, and only Up/Down then Cross forgets the preset. The menu test
binary passes all 268 tests.

## Not verified / not 1:1

- I did not look at the popup on screen. It is the same drawing path as Save Over's, which P229
  checked, and only its text is different.

## Next

none
