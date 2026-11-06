# 1 — machine picker (FINAL)

Task P315. 2026-11-06.

## What I did

The flier half of P315 is done: the picker is built from the manifests, picks are per seat, and the Fliers rows are gone from Settings. The rest of P315 is split off as P315a–P315g (see Next).

- **The offer comes from the manifests.** `flier_offers` (main.rs) lists MX Unleashed's two liveries of each flier first; `roster::original_fliers` describes them in code as one more pack. After them comes every skin in every installed machine pack's `mod.json` (`mods/machines/*`). Each skin gets:
  - its game, from `source.game` (`machine_packs::Pack::game`, new);
  - its name and about, from the new optional `skins[].name` / `skins[].about`, else "Plane 3" and its model's description;
  - its model.
  Two kinds of skin are listed but greyed, with why:
  - a kind openmx cannot fly ("openmx cannot fly a hovercraft yet");
  - a kind whose pack has no `.phy` for it.

  Nothing in the code names a pack. The old hard-coded MX vs. ATV skin table (`Machine::mxvatv_*`, `MxvatvFliers`) is gone. Its place is taken by `vehicle::PackFlier`, which reads a skin's psg, hull, rays and `.phy` by the paths its manifest gives.
- **The Garage has a Fliers tab** (`GARAGE_TABS` is now Rider, Bike, Gear, Fliers).
  - There is one row per kind on offer. Choices are grouped by game, MX Unleashed's first (`roster::flier_choices`).
  - Left and right step the seat's pick and skip skins that can't fly (`roster::step_flier`).
  - Under the row: the pick's game and its about.
  - With no pack installed, the last row says that more come in machine packs.
  - The rows are plain left/right choices, not dropdowns, like the other Garage rows.
- **Picks are per seat**, saved in garage.txt as `seatN.flier = <kind> <pack> <skin>` (`roster::Pick::fliers`). Presets and rider looks leave them alone. `--garage` and the front end pass 1P's picks to the ride as `Args::flier_picks`.
- **The ride flies the pick** (`ride_fliers`).
  - A pack's skin is built from its own files: model, collision, `.phy`, and `ModelOne`/`ModelTwo` figures. The second copy of the machine wears the same model's other skin.
  - MX Unleashed's second livery now draws as the first copy. Before, it only showed on the second copy.
  - A pick whose pack is gone, or that can't fly, falls back to the kind's first flyable skin, with a `fliers:` line saying so.
  - With nothing picked, the ride is exactly as before and prints no line. The mxvatv_fliers.rs default test still holds.
- **Removed:** Settings → Ride's Fliers and Flier Skin rows, and the `fliers`/`flier_skin` settings.
  - An older settings.txt is still read (`Settings::old_fliers`). `Menu::set_fliers` turns it into 1P's picks of the first flyable pack's skin at that place (the last one if there are fewer), then writes settings.txt without it.
  - `--fliers original|mxvatv` and `--flier-skin N` still work. `mxvatv` now means the first installed pack with flyable fliers. `N` is any place in that pack.
- **Online:** unchanged until P318. A ride is always MX Unleashed's fliers (`a.fliers = Some(false)` kept), and a peer's pick carries no fliers (`peer_looks.rs`).

From opentennis: nothing. Its menu has no per-machine picker.

## What the original does

Nothing decompiled. This is menu and mod plumbing. The flight itself is MD80b/MD80c's, unchanged.

## How it was proved

- **Renamed test pack.** I copied `mxvatv_fliers` to a scratch `mods/machines/renamed_fliers`. In its manifest I changed the id to `renamed_fliers`, the name, and `source.game` to "Test Game"; gave every skin a name such as "Kiowa4 (renamed)"; and added an unflyable `hovercraft` skin. Then I rode `--headless --track free09 --frames 30 --garage --mods <scratch>/mods` with a garage.txt picking `biplane renamed_fliers biplane3` and `kiowa renamed_fliers kiowa4` for seat 1:
  ```
  fliers: biplane Biplane3 (renamed) (biplane3), from renamed_fliers, model 2
  fliers: kiowa Kiowa4 (renamed) (kiowa4), from renamed_fliers, model 2
  biplane: flies on MX vs. ATV's biplane.phy, on its steps
  kiowa: renamed_fliers's, on its steps, drawn in kiowa4
  biplane: renamed_fliers's, drawn in biplane3
  ```
  With `biplane gone_pack biplane3` and `kiowa original kiowa2`:
  ```
  fliers: biplane biplane3 from gone_pack is not installed or cannot fly; biplane from original
  biplane: flies on its biplane.phy
  kiowa: MX Unleashed's, drawn in kiowa2
  ```
- **Menu shots** of `--menu --screen fliers --mods <scratch>/mods` on a private headless weston:
  - `context/shots/p315/fliers-tab-renamed-pack.jpg` (not committed): from a garage.txt with MX Unleashed's second biplane livery and the renamed Kiowa4. The Hovercraft row is greyed.
  - `context/shots/p315/fliers-tab-migrated-settings.jpg` (not committed): from a settings.txt holding only the old `fliers = mxvatv`, `flier_skin = 3`. It shows Biplane3/Kiowa3 picked. The menu then saved garage.txt with those two `seat1.flier` lines and settings.txt with no flier line.

  Both shots were taken before the dropdown fix, so the rows still show the ▼.
- **Tests:**
  - New `tests/menu.rs::the_fliers_tab_picks_a_renamed_packs_skins_per_seat`: the tab's rows, choices, greyed kind, the left/right steps skipping an unflyable skin, the pick reaching `Menu::picks` on Back, and the old-settings migration.
  - New roster unit tests: the garage.txt round trip, grouping by game, stepping over unflyable skins, and a missing pack falling back.
  - New `menu::tests::an_old_fliers_setting_is_read_and_never_written`.
  - New vehicle test on the livery order.
  - `tests/mxvatv_fliers.rs` (three rides each on the real pack) passes with its line strings updated to the pack id.
- **Gate:** `./kit check --all`: 5126 tests passed.

## Not verified / not 1:1

- **P2's own picks don't fly yet.** garage.txt keeps and the tab sets a pick per seat, but the ride builds one plane and one helicopter for every seat from 1P's picks. P2 flies 1P's pack, or MX Unleashed's. The brief's "ride lines for P1/P2" is not met → P315b.
- **No 3D preview.** The Fliers tab's stage still shows the rider and bike, not the picked flier turning → P315a.
- **No mid-ride Switch To Machine picker.** Switch To Machine is as before: the pick is read once when the ride is built. No skin-stepping tape was made, because skins can't be stepped mid-ride yet → P315g.
- **Bike class and skin in the picker, and handling per machine, not done.** The Bike tab and Settings → Ride's Handling / Handling From rows are unchanged → P315c, P315d.
- **Left/right step skins, not L1/R1.** L1/R1 stay the Garage's tab switch, which tests/menu.rs walks → P315f, a decision for the user.
- **The real pack has no skin names**, so it reads "Biplane 1" … in the picker. Its importer and modcheck aren't touched here; openmx-mods has unmerged worktree branches → P315e. modcheck checking the new fields is P315e too.
- **No menu shot of a flier row under the cursor**, with its note and the greyed row's reason. With no keystrokes on this desktop, the shots open on the Player row. The menu test asserts the notes and reasons.
- **Two lines still say "MX vs. ATV's"** whatever the pack: "biplane: flies on MX vs. ATV's biplane.phy" and the help text. They describe SLUS-21104's flight steps, which are the only non-MXU steps a pack flies today.
- **Online:** a peer's flier picks aren't sent, and online rides are MX Unleashed's (P318).
- **`--fliers mxvatv` with several flier packs installed** takes the first in folder order. It was ridden with only one pack in the folder.

## Next

The machine picker continues in:

- P315a: the 3D preview
- P315b: P2's own fliers
- P315c: bike class and skin in the picker
- P315d: handling per machine
- P315e: skin names and modcheck in openmx-mods
- P315f: L1/R1 or left/right, a question for the user
- P315g: Switch To Machine's picker mid-ride, with the tape
