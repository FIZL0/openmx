# 1 — pack fliers own model names (FINAL)

Task P325. 2026-11-16.

## What I did

- The bug: `mxvatv_fliers` names its models as the disc does (`biplane`, `kiowa`, `propconeblur`,
  `bladeblur` …) and its textures too (`Pitts_body01.psm`, `Kiowa_Blades.psm`). `Assets` keys models and
  textures by name, so whichever loaded first was drawn for both seats. Prefixing only the model name
  would still have shared the textures, so the textures are scoped too.
- `game_render::world`: `pack_key(pack, name)` makes `pack:name`; `split_key` splits it.
  `Assets::load_key` loads `name` under the whole key (through `load_as`) with a `scope` field set, which
  `answer_key` puts in front of every material/texture key while it loads. Plain names load as before.
- `Biplane::load_model` / `Kiowa::load_model` load the model by key and their cones / rotor parts under
  the same pack part. `Biplane` keeps `cones: [String; 2]` (`cone()`, `models()`); `Kiowa` keeps
  `parts: [String; 4]` (`bent()`, `models()`), and `Kiowa::found_model` rebuilds them off the key.
  `Kiowa::parts` now borrows its names from `self` instead of `&'static str`.
- `flier_refit::model_key(f)` is the pack skin's key. main.rs's four pack load sites (ride load ×2, the
  pause's refit, online pack planes) use it, the GPU upload lists use `models()`, and the thrown blades
  draw `drawn.bent()` (the pack's `bladebent` for a pack Kiowa, as it would have been alone).
- The `drawn in …` ride lines print the skin (`biplane3`), not the key, so the lines tests read are as
  before. `lot.mxvatv_kiowa` holds keys now (only `found_model` reads it).
- opentennis: nothing like it there (no packs of another game's machines); nothing taken.
- Copied `mods/machines/mxvatv_fliers` from slot s1 into this worktree (git-ignored) so the pack tests run here.

## What the original does

Not touched: drawing is ours. The original loads one game's files, so it has no name clash.

## How it was proved

- `crates/openmx/tests/pack_flier_models.rs`: the disc's `biplane` (P2's MX Unleashed Biplane) and the
  pack's Plane (1) (`mxvatv_fliers:biplane`) loaded into one `Assets`, in both orders, each drawn by the
  software path (`draw_model`, plane + cone). The two pictures differ by 1276 px (of ~3700 lit), and each
  plane's picture is the same whichever loaded first. Same for the disc's and the pack's `kiowa` with
  rotor parts: 1656 px apart (of ~9100 lit). The test first checks the two `.psg` files differ.
- `mxvatv_fliers`, `machines`, `prop_see_through`, `biplane_surfaces` (openmx) and `kiowa` (game-render)
  still pass, including P315b's two-seat picks and P315g's mid-ride switch.

## Not verified / not 1:1

- No full-shell shot of a two-player ride: the proof draws the two planes from the same loaders the shell
  calls, not out of a `--players 2` ride's frame. Reason: a headless two-seat ride with both in planes
  needs a tape per pad and a camera on each; the loader level is where the clash was.
- The GPU path was not shot: it keys models by the same names `models()` gives it and textures by index,
  so it follows `Assets`; not rendered here.
- I did not rerun the test on the old code to watch it fail; with the pack first, the old keys would have
  given both seats one model (same `models["biplane"]` entry), which is what the test rules out.
- The Garage's own flier view (`garage_view.rs`) loads into its own `Assets` and was left alone.

## Next

- none: done.
