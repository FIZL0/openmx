# 1 — machine picker (FINAL)

Task P315c. 2026-11-08.

## What I did

The Garage's Bike and Fliers tabs are now one tab, **Machines**. `GARAGE_TABS` is now Rider, Machines, Gear, and `TAB_MACHINES` replaces `TAB_FLIERS`.

- **Rows**, in order: Player, Bike Class, Bike, Paint, Driving Model, then one row per flier kind (Plane, Helicopter, any greyed kind such as a hovercraft), then one greyed row for each kind of machine openmx cannot ride (Quad, with the mxvatv_atv pack installed).
- **Manifest-driven offer** (`main.rs::machine_offers`, `roster::PackMachine`, `Menu::set_machines`). Next to `flier_offers`, every installed machine pack is read again for:
  - its bikes: skins whose `machine` is `bike`, each with a new `class` field (`machine_packs::Skin::class`, number or string, e.g. `250`);
  - each skin-less pack of a kind that is neither a bike nor a flier (the quads). These are listed once, by the pack's name.

  `flier_offers` now skips bike skins, so a bike pack's skins don't turn up as an unflyable "Bike" flier kind.
- **Bike rows** (`Menu::bike_rows`):
  - Bike Class lists the disc's classes, then any class only a pack has ("450cc (cannot ride)").
  - Bike lists the class's disc models, then the packs' bikes of that class grouped by game ("Test 250 (cannot ride)"). The note under it reads "MX Unleashed's own." or "MX Unleashed's own. Also <game>'s 250cc: openmx rides only MX Unleashed's own bikes yet."
  - Stepping still goes through `roster::Garage::turn` over the disc's catalogue, so pack bikes are listed but stepped over, just as unflyable fliers are.
  - Paint now lists its choices as well.
  - MX Unleashed's bikes are the "one more pack" the brief asks for, and they come off the disc's own catalogue (`roster::Catalogue`). Nothing in code names a pack.
- **Other kinds** (`Menu::other_machine_rows`, `Key::Machine`): one greyed row per kind. It names the packs that bring it, with their game as the note, and why openmx can't ride it ("openmx cannot ride a quad yet").
- **Stage**: it shows the bike on the Player row and the bike rows, and the flier whose row is under the cursor on a flier row. P315a showed the plane on the Player row; now the bike shows there. The stage's tap arrows now appear on the whole tab: on a flier row they step that flier's skin, elsewhere they step the bike's paint (`Menu::step_garage_machine`).
- **`--screen machines`** opens the tab. `--screen bike` and `--screen fliers` still open it too (`MACHINES_WAS`), so old shot commands and the P213 shot test keep working.
- garage.txt is unchanged. The bike pick stays `seatN = bike set rider` and the fliers stay `seatN.flier = …`. A pack bike can't be picked yet, so it has no line (→ P315h).

From opentennis: nothing. It has no machine picker.

## What the original does

Nothing decompiled: this is our front end (the menus are ours to design). The disc's own Garage has separate bike and flier screens. Merging them into one picker is the user's ask (P315).

## How it was proved

- **Tests:**
  - New `tests/menu.rs::the_machine_picker_lists_pack_bikes_and_round_trips_garage_txt`, on the real disc's catalogue with a hand-made offer of two pack bikes (250, 450) and a quad pack. It checks:
    - the tab's eight row labels;
    - the class choices ending "450cc (cannot ride)";
    - the Bike row's note at 125cc and at 250cc, and the pack's 250 listed last at 250cc with no 450 there;
    - stepping right through every Bike choice never lands on a pack bike;
    - the Quad row greyed, with its why and its game;
    - the stage's right arrow on the Paint row stepping 125sx → 125sx_one, with no flier shown;
    - the plane row stepping to biplane2;
    - Circle, then the pick written to garage.txt (`seat1 = <bike> <set> thq1`, `seat1.flier = biplane original biplane2`) and read back equal.
  - New `main.rs::machine_offers_test`: a scratch pack's manifest (`machine: bike`, two skins, `class` as a number and as a string) plus the real `mods/machines/mxvatv_atv` give the expected offers, and `flier_offers` lists no bike skins.
  - Updated: the fliers test (now on the Machines tab, eight labels, no flier shown on the Player row), the tab-walk test (three tabs), and the P213 rows test and per-seat ride test (now tab "Machines").
  - `./kit check -- -p openmx --test menu`: 279 passed. The full gate runs in `./kit done`.
- **Menu shots** (`target/debug/openmx --menu --screen machines --size 1920x1080 --shot`, on a private headless weston). Not committed:
  - `context/shots/p315c/a.png`: a scratch folder with no mods. Rows Player … Driving Model, Plane "Biplane", Helicopter "Kiowa". The bike turns on the stage, with the arrows under it.
  - `context/shots/p315c/b.png`: `--mods` holding copies of mxvatv_fliers and mxvatv_atv plus a scratch `test_bikes` pack, with a garage.txt of `125sx_one`, `biplane mxvatv_fliers biplane3` and `kiowa original kiowa2`. It shows Paint "One Industries", Plane "Plane 3", Helicopter "Kiowa, Second Livery" and a greyed "Quad — MX vs. ATV Unleashed quads".

## Not verified / not 1:1

- **A pack's bike can't be ridden or picked.** No bike pack exists in openmx-mods. Riding one needs a ride path for a pack's model, `.phy` and collision, which openmx doesn't have. So pack bikes are listed greyed and stepped over. The manifest field (`skins[].class`) is my own choice, because no importer writes bike skins yet. → P315h (later).
- **No shot shows the pack bikes in the side panel.** They show as choices only with the cursor on the Bike row. With no keystrokes on this desktop, shots open on the Player row. The menu test asserts the choices and note.
- **The quad row lists the pack, not its builds.** mxvatv_atv has builds and no skins, so it's one greyed row named after the pack, not a choice per quad.
- **A bike pack with builds but no skins is not shown.** It has no class to list it under. No such pack exists.
- **The stage no longer shows the plane on the Player row.** P315a showed the first flier there; on the merged tab the Player row shows the bike. The P315a test was changed to match.
- **The arrows stepping the bike's paint were exercised only through `Menu::click`** in the menu test. They were not tapped on a phone or in the browser, the same gap P315a recorded.
- **Handling per machine (P315d) and Switch To Machine's mid-ride picker (P315g) are not here.** The mid-ride picker will need these same bike rows.
- **Shots are on the GPU path only** (llvmpipe under weston). The software path was not shot.

## Next

none: P315d/e/f/g/h carry the rest.
