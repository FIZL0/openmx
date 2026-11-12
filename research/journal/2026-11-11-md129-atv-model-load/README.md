# MD129 — atv model load

2026-11-11. Task: `./kit plan MD129`.

The quad loader's two model-side inputs, read off the R quad's chassis model (250r_atv_chasis.psg out of
MXVSATV.PSR's `Veh\vehicletype_1.res`) at the quad's spawn point and heading, so `quad::Load::read_model`
needs no RAM image: `quad::Model` (the psg's nodes), `Model::mounts` / `Model::half_box`, and the two
frames `spawn_frame` (heading made unit) and `heading_frame` (heading as kept). The mounts and sweeps are
bit-exact on all four C28 images. The box is one float under the record's in `z`, so four record words
(inertia x, y and their inverses) are 1–2 floats off. That gap is filed as MD134.
