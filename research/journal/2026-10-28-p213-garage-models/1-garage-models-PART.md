# 1 — garage models (PART)

Task P213. 2026-10-28.

## What I did

- **The stand** (`crates/openmx/src/garage_view.rs`, new): the Garage draws the seat's pick as the ride
  draws it — the same `game-render` chassis and parts (`load_repainted` with the pick's livery), the same
  skinned rider (`Body::load`, the kit's repaint of the suit with the look: colours, gear set, jersey/pants,
  name and number on the plate texture), placed on the chassis nodes exactly as `dress_peer` does. It is
  re-dressed only when the pick changes (`Stage::dress` compares the whole `Pick`), so every row of every
  tab — class, bike, paint, gear set, jersey/pants, pieces, colours, name, number — shows on the model,
  and the Player row switches whose pick stands there (per seat).
- It turns slowly (0.4 rad/s, wall clock through the menu's own tick), lit by a fixed sun, framed by its
  bounding sphere, top right of the page where the original's stands (`GARAGE_STAGE`, 59..79.6 x
  1.6..24.8 em). The side panel moved down under it (`side = 25.2`).
- **GPU**: `Renderer::render_over` (game-render `gpu.rs`) draws a scene's actors only, over what the target
  already holds (load, not clear; depth cleared) — no ground, sky, props or particles — so the menu's
  sprites go first and the bike over them, in the stage's viewport.
- **Software**: the model into a stage-sized frame, its covered pixels copied over the page; the page
  under the stage is kept so each turn restores it first. The software menu was redrawn only when its
  look changed; now a turned stand presents too.
- A `--shot` stands the model at the turn's start (tick 0) so two shots of two picks compare.
- **Names** (`roster.rs`): the original names its bikes only by logo images (`ui\tex\logo_*` in
  `UI\ui.res`, chosen by `logo=` per bike in `bike\bikes.dat`). I decoded every logo and read them
  (`BIKE_LOGOS`: 39 logo → maker, model, e.g. `logo_ktm_125sx` → KTM 125 SX); the rider set names
  likewise (`RIDER_NAMES`: gear makers and the pros of `rider\riders.dat`, e.g. tld → Troy Lee Designs).
  The rows take the original's labels: Bike Class, Bike, Driving Model (Race Physics / Pro Physics), Gear
  Set, Jersey/Pants. Unknown archives fall back to the old names.

## What the original does

The garage turns the picked bike with the rider beside/on it in the right half of the screen, re-dressed as
rows change; its names are drawn as logo images, not text. (Read from the menu's `.res` files, not from code:
no function was decompiled for this task.)

## How it was proved

- `crates/openmx/tests/menu.rs` `the_garage_shows_the_picked_bike_and_rider_in_its_paint`: two scratch
  stores (kit.toml + disc symlink + garage.txt), 1P on `125sx` and on `125sx_one`, each a 640x360
  headless `--shot` of the Bike tab: the stage holds many grey (unsaturated) pixels where the page's own
  orange has none, and the two paints differ in over 500 stage pixels.
- `the_garage_rows_carry_the_originals_names`: the real catalogue's Bike tab reads Bike Class 125cc,
  Driving Model Race Physics, a Bike starting with its maker; the Rider tab has Gear Set and Jersey/Pants.
- Looked at by eye: software shot and a headless GPU shot (temporary hook, removed) of the Bike tab both
  show the bike and seated rider, livery and plate visible.

## Not verified / not 1:1

- The rider sits on the bike in the ride's `get_ready_breathe` pose (frame 0), still; the original's
  garage animation clips (`.mlb`) were not read, so its pose, idle motion and placement are not matched.
- Turn speed, camera angle/distance, light and the stage's exact place were set by eye, not measured
  against a PCSX2 shot of the garage.
- Names are text, not the original's logo images; maker/model spellings (N-Style, Moto XXX Racing, the
  THQ/One Industries sets) were read off stylised logos and may differ from the original's own wording.
- Dressing runs on the menu thread: a pick change loads the bike and rider synchronously (a short hitch).
- The web build does not draw the stand (`garage_view` is mounted in the desktop shell's main.rs only).
- The GPU path was checked only headless/offscreen by eye; no test covers it (shots are software).
- The side panel under the stand fits fewer choices than before.

## Next

none — done. Landed with `--skip-audit`: `./kit audit` reports only the 7 old addresses in plan/DONE.md (other tasks' biplane_tire_* names), none in this task's files.
