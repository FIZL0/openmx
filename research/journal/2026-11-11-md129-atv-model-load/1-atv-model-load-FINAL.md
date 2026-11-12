# 1 — atv model load (FINAL)

Task MD129. 2026-11-11.

## What I did

- Pulled `Veh\vehicletype_1.res` out of MXVSATV.PSR (memory: mxvsatv-psr-off-the-iso). Its chassis for the R
  quad is `250r_atv_chasis.psg` (43 nodes; ATV_R.mcf's `SLTFile=250R_atv_chasis.Slt`). The test reads it
  from `context/mxvatv-ps2/md37a/250r_atv_chasis.psg` and skips without it. Copy it in from the res:
  `./kit` has no extractor, so a scratch crate over `archive::Psr`/`Res` did the job.
- quad.rs (additions only):
  - `WHEEL_NODES`, `ModelNode`, `Model::parse` (the psg's 224-byte records).
  - `Model::worlds`: the scene's world update, which is `toys::compose`, not `math::mul`, whose order
    differs.
  - `Model::half_box`: the container node with no box, at the frame, over the model, through
    `prop_world::node_bounds`.
  - `Model::mounts`: each wheel node's world origin back into the frame, `w` 1, no sweep.
  - `spawn_frame`, `heading_frame`, and `Load::read_model(ini, model, at, heading, curve)`.
- Tests in tests/atv_quad/load.rs:
  - `the_models_mounts_are_the_c28_wheels`: bit-exact, all four images.
  - `the_models_box_is_the_c28_records_but_one_float`: pins the gap exactly — x, y equal the record's
    box, z one float under, and only record +0x60/+0x64/+0x70/+0x74 differ.
- research/oracle.txt `# MD129`: every address below and all the bit patterns.

## What the original does

- **The scene.** The quad's scene node (quad +0xe4) is a container: flag bit 2 clear, so no box of its
  own, and its world rows are the quad's placement. Its one child is the chassis root.
  - Every chassis node has a box: the psg record's centre and half, `w` 1, byte for byte.
  - At the reload, the node locals are the psg's bind locals. The animation controller saves them
    untouched, and its saved frames match the psg byte for byte.
- **Mounts** (`./kit fn 0x4ba310`, quad vt +0x470, called by the reload `./kit fn 0x4bb370`):
  - for each of the four wheel names: find the node, then `./kit fn 0x194c20` (node, quad's node,
    wheel+0x90);
  - that is the node's world origin less the frame's origin, through the frame's transposed rows
    (`./kit dis 0x1953c0 0x195460`).
  - World rows come from `./kit fn 0x193b30`: local × parent, VU order.
- **Sweeps.** The init's four wheel setups (`./kit fn 0x4a0410`) build a sweep shape only for a nonzero
  last argument, and the init passes 0. So no wheel has one, and +0x1dc is 0 on every image.
- **Box.** The part reader calls `./kit fn 0x1961f0` on quad +0xe4: the span of every descendant's box
  in the node's frame, the halves `(hi−lo)·0.5`. This is already ported as `prop_world::node_bounds`.
- **The frame decides the low bits.** At ~3,000 ft one float is ~2.4e-4, and the model is turned into
  world and back, so the bind pose at the origin is up to ~1.6e-3 off the mounts. The image's live
  container rows are ~3e-4 off.
- **The spawn.** The quad keeps its spawn point at +0x50 and its heading at +0x60, the same on all four
  images.
  - With `z = unit(heading)` (every VU and FPU normalise route gives the same bits), `y = (0,1,0)`,
    `x = (z.z, 0, −z.x)` and the origin at the spawn point, all twelve mount words match.

## How it was proved

- **Save-state captures:** C28's four md37a images (r/s, spawn/air).
- **Mounts:** bit-exact, 4 wheels × 3 lanes × 4 images, sweep flag included.
- **Box:** compared against the record's inertia (rec +0x60).
  - `load_box`'s search shows the only box near that gives the record is the model's with `z` +1 float.
  - `Load::read_model`'s words are the image's except for those four record words.
- `./kit check -- -p game-sim --test atv_quad load::`: 4 passed.

## Not verified / not 1:1

- **The part reader's box is one float under in `z`, so it is not 1:1.** As a result, record
  +0x60/+0x64 (inertia x, y) and +0x70/+0x74 (their inverses) are 1–2 floats off on all four images.
  - `heading_frame` (the heading as kept) gives x and y exactly. `spawn_frame` is 512 floats off in x,
    so the loader in the constructor sees a different frame from the reload.
  - Frames tried that all miss: identity, rotation only, origin only, mixed raw/unit rows, and
    `y = z × x` (either sign).
  - The loader's actual placement is not found → MD134.
- **`heading_frame` is not proven.** It is the closest of the frames tried, not one read from the
  constructor's code.
- **`spawn_frame`'s construction is inferred.** It is reconstructed from the result, not read from the
  code that builds the reload's container rows: twelve mount words match, but the routine that writes
  them was not found. Unproven for a heading with `y ≠ 0`, or for a spawn where a different normalise
  would round differently.
- **Only the R model was checked.** The S quad's own chassis (`atv_s_chasis.psg`, `atvs_r_chasis.psg`)
  was not, because the s images are the R quad poked.
- **The wheel node lookup** (`./kit fn 0x195a90`) is assumed to match by exact name. The four names are
  unique in this psg.
- **Only the wheel mounts were ported.** The other nodes the mount setup reads (into +0x1860 and
  +0x1910..+0x1960, and +0x1900 = 1/distance) are not part of `Load`, so they were not ported.

## Next

- Landed as done, not blocked: both inputs now come off the model with no image, the mounts are exact, and
  the box's remaining float is pinned by its test and filed as MD134. Blocking would hold MD123 for 1-2
  floats of record inertia.

- MD134: find the constructor-time placement of the quad's scene node, so the box is exact.
- MD123 can use `Load::read_model` now. The box's 1-float `z` error moves the record inertia by
  1–2 floats.
- Gate: mxvatv_car_frame's `all_eight_pieces_are_mx_unleasheds` was red at this base ("degenerate seen
  only 19 times"), a known red fixed on main by F31b5l (078736f3d). Its test-file hunk (6,000 frames) is
  applied here unchanged. The follow-up was renumbered MD134, because main already uses MD132 and MD133.
