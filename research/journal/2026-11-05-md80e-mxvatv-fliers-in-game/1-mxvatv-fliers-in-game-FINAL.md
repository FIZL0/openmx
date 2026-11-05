# 1 — mxvatv fliers in game (FINAL)

Task MD80e. 2026-11-05.

## What I did

- **The switch is a setting.** The Tricks tab has a **Fliers** row (MX Unleashed / MX vs. ATV) and a
  **Flier Skin** row (Skin 1–4, with a note saying model one or two). It's saved in settings.txt as
  `fliers = mxvatv` / `flier_skin = N`, and only off its default, so older files are written unchanged.
  The flags are `--fliers original|mxvatv` and `--flier-skin 1..4`. Fliers is greyed out when the mods
  folder is off. Default: off.
- **Where it applies.** `ride_fliers` (main.rs) turns it on only offline with no room handling or tricks,
  and only when a machine pack with `machine: "fliers"` (openmx-mods' `mxvatv_fliers`, MD80d) is
  installed. Online, rooms and the lobby always fly MX Unleashed's fliers: the switch isn't on the netplay
  wire (MD100). It prints `fliers: MX vs. ATV's, from <id>, skin N (model M)`.
- **The biplane.** When it's on, the biplane is built from the pack's `Veh\biplane.phy`:
  - read with `Plane::read_mxvatv_bytes` for the picked skin's model;
  - flown on SLUS-21104's step (`Flight::new_mxvatv`, MD80b) via the new `vehicle::mxvatv_flight`. That
    function still runs the Game Modifiers' plane edits, and `retune` keeps the switch;
  - given the skin's `Col\<stem>F.col` hull, `M.col` rays and spawn lift;
  - drawn in the skin's model via the new `Biplane::load_model`, with the prop cones too.
- **The Kiowa.** It's built with the new `KiowaRide::new_mxvatv` from the pack's `Veh\kiowa.phy`, with the
  pack's rays, `Anim\<skin>.psg` and hull. It's flown with `Kiowa::tick_mxvatv` (MD80c): `Heli.mxvatv`
  carries the pedal, and `KiowaRide::step` picks the step. It's drawn in the skin's model via the new
  `Kiowa::load_model`/`found_model`, from the pack alone (MD80d's pack has every part and texture;
  `Res` isn't Clone, so it can't be put next to the disc archive).
- **The garage and Switch To Machine** both go through `Lot`, so both get MX vs. ATV's copy with no
  change of their own.
- **A second copy's skin.** Livery 0 is the picked skin and livery 1 is the other skin of the same model
  (`vehicle::mxvatv_skin`: 1↔2, 3↔4). So every copy flies one model's figures, and the 0/1 livery
  meaning stays as it is everywhere else.
- Nothing from opentennis applies (it has no machines).

## What the original does

SLUS-21104 offers four skins for each flier, over two models: skins 1/2 read the `.phy`'s `ModelOne*`
keys and skins 3/4 read `ModelTwo*`, each skin with its own model and collision pair. The steps
themselves are MD80b's and MD80c's, oracle-proved there.

## How it was proved

- **Unit tests in vehicle.rs:**
  - `a_second_copy_of_an_mxvatv_flier_wears_the_other_skin_of_its_model`;
  - `mxvatv_fliers_fly_slus_21104s_steps_and_mx_unleasheds_do_not`, which checks that a plane and a Kiowa
    built from minimal ini text carry the switch only on the MX vs. ATV path.
- **Unit test in menu.rs:** `the_fliers_setting_is_off_by_default_and_kept`. It checks that a default
  file says nothing, all four skins round-trip, and bad words fall back to the default. The full
  `Settings` literal in tests/menu.rs now sets both fields.
- **crates/openmx/tests/mxvatv_fliers.rs** runs the binary headless on free09 with hand-written tapes
  (Cross held, `switch` into the machine and back):
  - **Off:** unset and `--fliers original` print no MX vs. ATV lines. The Kiowa's leave line is
    identical in both (it climbs >100 ft and drifts <10 ft, MX Unleashed's), and the biplane's
    end-of-ride `plane hash` is identical too. The existing MX Unleashed replay and hash tests are in
    the full gate and pass unchanged.
  - **On, Kiowa skins 2 and 3 (models one and two):** drawn in kiowa2 / kiowa3. Each climbs >100 ft
    with <10 ft drift, and its leave line differs from MX Unleashed's.
  - **On, biplane skins 1 and 4:** prints "flies on MX vs. ATV's biplane.phy, on its steps" and is drawn
    in biplane / biplane4. Model one's and two's plane hashes differ from each other and from MX
    Unleashed's.
  - The first try checked the status line's mph. Those lines come on wall-clock time, so one run had
    none, and I switched to the hash.
- **By hand (headless):** the Kiowa climbed 203.9 ft against MX Unleashed's 254.1 ft on the same tape,
  and the biplane did 37.6 mph (skin 1) against 43.7 mph (skin 3).
- **`./kit check --all`:** everything passed except `web::the_browser_rides_on_the_gpu` (webgpu on a phone
  0.3 fps) and `texture_reload::a_file_rewritten_mid_ride_is_drawn_with_the_variable_set`. Both are GPU
  timing tests this change doesn't touch, and both passed when rerun alone.

## Not verified / not 1:1

- **MD80a's flights are not replayed** in the shell. The MD80a capture csvs are gone; MD101 (biplane)
  and MD104 (Kiowa) re-record and replay them. "Each flies its MD80a replay" is proved only as far as the
  steps' own oracles (MD80b/MD80c) go. The shell tests prove the MX vs. ATV step and files are what fly,
  but not that a flight matches a capture.
- **Cameras, engine sounds and particles** are still MX Unleashed's (MD105).
- **The Kiowa's thrown blades** (wreck) are still the disc's blade files (MD106).
- **The front-end garage (garage_view)** shows no MX vs. ATV skins. Only the in-ride garage and pause
  Switch To Machine give them (MD106).
- **Both copies use the picked skin's collision pair** (hull and rays), not each livery's own. Skins of
  one model share figures, but their col files weren't compared.
- **The second copy's skin rule** (1↔2, 3↔4) is my design, not the original's: the original has no
  two-copies-on-a-map case to copy.
- **Offline only:** online and rooms always fly MX Unleashed's fliers (MD100, noted for Heli's switch
  too).
- **The Assets cache is keyed by model name.** A pack model named like a disc model (`biplane`, `kiowa`)
  would be shared with the disc's in the same ride. The two are never loaded together (one or the other
  per ride), but a ride that loaded both would draw whichever came first.
- **Not watched on screen.** No shot or video of the MX vs. ATV models flying was taken; the draw is
  proved only by the "drawn in" lines (the models and their textures loaded).
- **Kiowa skins 3/4's `…01` seat nodes are unused.** The Kiowa's pilot doesn't sit on seat nodes in the
  shell.

## Next

Lands as is. MD101/MD104 (replays), MD105 (cameras, sounds, particles), MD106 (front-end garage, blades)
and MD100 (online) are the follow-ups.
