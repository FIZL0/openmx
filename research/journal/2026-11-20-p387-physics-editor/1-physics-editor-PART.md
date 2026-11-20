# 1 — physics editor (PART)

Task P387. 2026-11-20.

## What I did

- `crates/openmx/src/phy_editor.rs`: 25 knobs, each a percent scale (10–400%, 5% steps) on a group of the
  bike `.phy`'s own keys, under the brief's groups: Tires (grip, rolling drag, size), Suspension (front and
  rear spring, damping, travel), Crashes (speed, angles), Engine (power, rev range, engine braking, clutch),
  Drivetrain (final drive, gears), Controls (throttle, steering lock and response, lean speed), Body (gravity,
  weight, inertia, air drag), Grip (traction, brakes).
- The knobs rewrite the file's text: new `Ini::with_values` / `Ini::text` in `game-sim/src/phy.rs`. The
  ported loader then reads the result, so with every knob at 100% the bike is the disc's file read as always.
- Save writes `mods/handling/phy_<name>/<bike>.phy` plus a `mod.json` (kind `handling`, the bike, the knobs).
  Load lists the saved mods for the ridden bike and puts their knobs back over the disc's file.
- Menu: Settings › Ride has a "Physics Editor" On/Off switch, off by default and kept in settings.txt. With
  it on, an "Edit Physics" row opens the Physics Editor page (`menu_physics.rs`). The page has one slider
  per knob showing the percent and the file's first value, plus Save as Mod (keyboard), Load Mod and
  Reset All. The page only queues asks. The shell owns the file.
- Shell (`main.rs`): every frame, with the switch on, it takes the asks. On a change it swaps player 1's
  stock bike for the edited one through `Mods::resim`, the same path a mid-ride modifier change takes. So
  the ride retunes where it is, and the edit applies from the next tick. Edits carry to the next ride on the
  same bike.
- Held (knobs dimmed, nothing changes) in three cases: an online ride, a tape (`--watch`/`--tape`), or a
  bike that isn't the disc's file as read (MX vs. ATV/Untamed handling, a trick pack laid over it).

## What the original does

Nothing: the original game has no physics editor. The idea is MXA's Physics Studio's knob groups, taken
from the brief only. `context/mxa` is not in this checkout, so I read none of MXA's files.

## How it was proved

`crates/openmx/tests/phy_editor.rs` (5 tests) on the harness's ridden stand-in bike:
- every knob at 100% matches the loader's `Phy` and rides 240 ticks hash-identical to the plain file
- Engine Power at 120%, swapped in at tick 90, changes the very next tick's hash, and the bike is
  faster a second later
- each knob moves only its own keys, and only ones the file has
- a saved mod reloads to the same knobs and name, and its file rides hash-identical to the editor's bike
- the held rule

`tests/menu.rs` `the_physics_editor_is_off_by_default_and_asks_the_shell`: the switch is off, then on and
saved; the page opens on its first knob; presses queue asks; rows dim where the file lacks a knob or the
bike is held.

`./kit check --all` is green except for two reds outside this change:
- `untamed_atv`: s1's installed pack predates MD251's camera
- game-render `whip_tricks` 360s: tricks pack data; this task only adds two `Ini` helpers to game-sim

## Not verified / not 1:1

- Riding an edited bike online "unless every seat has it" is not done. The editor is held on every online
  ride, and an edit never reaches a lobby ride. A saved mod is a P318 pack, but making each seat build its
  bike from it is filed as P387a.
- Not looked at in a live window (no screenshot of the page). It is covered by menu tests only.
- Only player 1's bike is edited. Split-screen players 2–4 ride their own files.
- Held under MX vs. ATV/Untamed handling and trick packs, because those bikes are not the disc's file.
- The knob-to-key groups are mine, from the brief's names. MXA's own groups (MxaPhysicsKnobs.cs) were not
  read, since `context/mxa` is absent here.
- A loaded mod's knobs are re-laid over the disc's file. The saved `.phy` is kept for the lobby (P387a) but
  not read back. A different disc file would give a different bike.

## Next

P387a: ride a saved handling mod online where every seat holds it.
