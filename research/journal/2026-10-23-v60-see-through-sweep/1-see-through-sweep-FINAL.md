# 1 — see through sweep (FINAL)

Task V60. 2026-10-23.

## What I did

- Wrote a survey tool, `crates/game-render/src/bin/see-through.rs`
  (`cargo run --release -p game-render --bin see-through -- [--all | --show <track> <mat> | --res <names…> | --list | --where <track> <word>]`).
  For every material on every freeride, supercross and national track, plus every `Bike\` and `Rider\` archive, it prints:
  - the .psm's alpha keys;
  - the texture's alpha histogram and the vertex alpha;
  - the `Surface` the port builds;
  - a SEE flag with the share of mapped texels that would be cut or blended.
- I read every flagged material against the original's material builders. Two rules break, and both are fixed:

| material (where) | .psm flags | port drew | rule broken |
|---|---|---|---|
| nat07_plow_body, steel_door (nat07/nat10), cr-250_plasticsall (nat10 actors), 125sx_engine (125sx bike), thq_helmet_01, thq_goggles_01 (thq1 rider), and every other `Vu1StripDiffuseGlossySpecularMap` pass | `totalTextureMaps = 2`, `alphaTestOnOff_1 = 0`, base alpha = gloss mask | holes wherever the gloss mask is 0 (bare `alphaTestOnOff` missing → the default GREATER 0 test) | **two-texture passes read only the `_1`-suffixed keys** |
| free07_door, free07_rubble, free07_windw, free07windw2 (free07 cabins and car) | `alphaBlendingOnOff = 1`, `inputColorA = 2`, `inputColorB = 2`, `inputColorD = 0` | an alpha mix: windows and a far cabin show the sky through them | **(A−B)·C+D with A = B and D = Cs is the texel itself, opaque** |

- Fixes:
  - `Material::parse` (scene.rs): when `totalTextureMaps == 2`, the alpha-test, blend, zWrite and wrap keys are read with a `_1` suffix.
  - `Surface::of` (world.rs): a blend of `[a, a, 0]` is no blend.
- Neither rule depends on the renderer, so both paths are fixed at once.
- Trees and bushes are left to V58.

## What the original does

- `./kit fn 0x17e1b0` reads a ShaderPass. Line ~150 sets a flag bit in the pass (+0x42) when `totalTextureMaps == 2`. That bit picks the builder:
  - `./kit fn 0x17eb00` (0x120-byte descriptor) builds a one-texture pass. It looks up the bare keys through `./kit fn 0x14b6f0`.
  - `./kit fn 0x17f4b0` (400-byte descriptor) builds a two-texture pass. Every key it looks up for the first texture is the `_1` form: alphaTestOnOff_1, alphaTestMethod_1, alphaReferenceValue_1, processingMethodWhenFailedInAlphaTest_1, alphaBlendingOnOff_1, inputColorA/B/C/D_1, zWriteDisabled_1, wrapMode*_1. A bare `alphaTestOnOff` in such a pass is never read.
- Both builders start from the same defaults descriptor (first_recording RAM 0x4f9870):
  - TCC 1 at +0x48, TFX 0 (modulate) at +0x4c;
  - blend A0 B1 C0 D1, FIX 0x80, at +0x5c..+0x6c;
  - ATE 1, ATST 6 (GREATER), AREF 0, AFAIL 0 at +0xb4..+0xc0;
  - depth test 2 at +0xcc.
- The GS blend is (A−B)·C+D. free07's [2,2,0] is (0−0)·As+Cs = Cs: the texel is written as is.

## How it was proved

- **Disc test, both renderers.** `crates/game-render/tests/solid.rs::the_materials_v60_found_hide_what_is_behind_them` covers all 10 materials in the table, on software and on the GPU ("checked on the GPU: 10 of 10"). Each material:
  - has no blend and cutoff 0;
  - has alpha-0 texels;
  - shows the backdrop through 0 of its solid pixels.
- **The test fails on the old reading.** The glossy materials must show through under a cutoff of 2 wherever more than 5% of their texels are empty, so steel_door, at 1.4%, is skipped. The cabins' blended picture must differ from the new one in over half of their pixels.
- V54's train test still passes ("6 of 6").
- Two unit tests in world.rs cover the `_1` rule (plow, bare-key, suffixed, one-texture cases) and the A = B blend.
- **Beside a screenshot.** The original's rider from behind (PCSX2 copy 3, save slot 3, first_recording's free09) draws the goggle strap as a solid red band across the helmet. The port before the fix cut the strap out; after the fix it is drawn (thq_helmet_01 / thq_goggles_01).
- **Port shots only for free07.** Port shots of free07 at `--trace-at 4321,2900` before and after, GPU and software:
  - before: the cabin windows and a far cabin's window show the sky;
  - after: they are solid.
  - The shots are in context/shots and target/tmp/shots, not committed.
- After the fix, the survey finds no blend or cut on any of the 2,091 bike and rider materials, except the intended ones: sprockets and the ghost bikes.

## What I took from opentennis

Nothing. I did not consult it. Both rules came straight from the original's builders.

## Not verified / not 1:1

- **free07 not seen in the original.** Every motocross save state is free09 (see memory), so the cabins' [2,2,0] fix rests on the GS formula and the port's shots, not on a screenshot of the original.
- **A0 B0 windows.** sx04/sx10/sx20 stadiumtopwindows, sx06/sx15 windows and sx14 lightgrid blend A0 B0 with D left at its default 1 (Cd). In the original they write depth and no colour. The port still alpha-mixes them, so they are drawn faintly. Not fixed: they are not see-through-vs-solid, and the opposite error would be invisible geometry. No follow-up filed.
- **Multiplicative shadow blend.** 160 materials use [1,2,2] (a Cd·As multiply). The port draws them as an alpha mix. These are ground decals, which are another V task's.
- **A material's second ShaderPass is ignored.** Its MaterialPassStart/End looks like a far/LOD range. Plain `Vu1StripDiffuse` second passes would take the default test, so the original may cut holes at a distance where the port does not. Not checked.
- **Small alpha-0 cut-outs not shot in the original.** They follow the default test and are holes in the original too, by the builder, but none was compared by eye:
  - sh_hanger a/b/c hangars (0.4–1.1% of texels);
  - handsaw, bulldozershovel (6% at sx), tiki_torch, spitpipe, rusted_pipe_b;
  - gas_station_sign_b (17.8%), sh_wgnwhl, nat08_bronco.
- **Vertex alpha and TFX modulate.** The alpha test runs on texel × vertex alpha in the original. The port tests texel alpha alone.
- **Rare keys.** inputAlphaC = 1 (Ad) appears only in later passes, and no FIX C appears on any track. Neither is handled differently.
- **Unchanged from V54.** V54's empty-value caveat stands. The 32 cut-off for a mesh with no .psm is unchanged.

## Next

- none; the follow-ups are the Not verified items.
