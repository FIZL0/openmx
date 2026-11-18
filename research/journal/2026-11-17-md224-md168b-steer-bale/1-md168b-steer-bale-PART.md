# 1 — md168b steer bale (PART)

Task MD224. 2026-11-17.

## What I did

- **A new capture, md224 `steer.csv`** under `context/capture/md224/` (205 rows, not committed).
  - Recipe: `research/md224/play.sh` (MD147's play.sh, outputs under md224) with `research/md224/quad.toml`
    (MD147's fields plus `rec`, `rec_list`, `bale_rec`, `bale_list`, `bale_body`).
  - Run: copy 2 at `NominalScalar = 0.3` (the recorder misses rows at full speed), then
    `./kit pcsx2 with -- bash research/md224/play.sh <pid> research/md147/steer.pad context/untamed/md147/spawn.p2s 0x01be4760 215 steer`.
  - Rows 0–204 equal MD147's steer except the torn rows 166 and 198. Rows 199–204 agree bit for bit
    between two runs.
- **quad.rs: `Knockable`, `RecordAmong`, `Ride::record_among`, `Ride::step_among`.**
  - A knockable body is its `f.col` mesh at its node's rows, plus its own collision record.
  - The quad's world rays strike it through the ATV ray probe (`ray_probe::pair`, rays at the scene
    node's frame, one contact per ray). The hit names the body's record as its other. The pass files
    the mirror (kind 1) on the body's record.
  - `frame_met` now goes through a new `frame_among` with no knockables, so every other caller runs
    as before.
- **Test `untamed_quad_record_strikes_the_bale_on_md224_steer`** (untamed_race_quad.rs).
  - It steps steer to tick 197 with the bale, then checks the quad's record list (kind, ray, normal,
    point, other) against row 197's words. Then it checks the bale's list's first two entries (the mirrors).
  - The bale comes from the spawn image: `BALE` = its record, and the frame is its node's rows
    (node at record `+0xe8`, rows at `+0x60..+0xa0`).

## What the original does

- **Untamed's record list entry differs from SLUS-21104's** (`0x90` bytes either way):
  - kind is a u16 at `+0` (0 own, 1 mirror), with `0xffff` in the upper half;
  - `+0x08` holds a pointer (`0x01be71e0` on the quad's own hits);
  - ray `+0x10/+0x20`, normal `+0x30`, point `+0x40`, arms `+0x50/+0x60`, relative velocity `+0x70`;
  - **the other record is at `+0x80`**. On the bale's list, `+0x84/+0x88` hold stale words.
- **Tick 197** (row 197 is read after the record pass and before the step): the quad's list holds
  2 own hits, from riding rays 9 and 13, on `TripleBalef.col`, other = the bale's record. The bale's
  list holds 11: the 2 mirrors first, then its own 9 terrain contacts.
- **Tick 198**: the quad holds 7 entries: 1 own hit, then 5 mirrors from the bale's rays
  (`TripleBalem.col`, normal ≈ (0.998, 0.064, 0)). So the bale's pass runs after the quad's in a frame.
  The quad's velocity x is already +2.297 and the bale flies at (-20.19, 10.92, 11.89).
- **Ticks 199–204**: no contacts. The bale flies under gravity (y velocity −1.0733 a tick).
- Bale: record `0xaac3d0`, body `0xaaa6c0`, mass 7.857, inv_inertia (0.2905, 0.02989, 0.02989).

## How it was proved

- A bit-exact compare against a save-state capture: row 197 of the md224 steer capture.
  The quad's 2 entries (16 words each, plus kind and other) and the bale's 2 mirrors match.
- On the way, MX Unleashed's bundle sweep (`world::Props::sweep`) got the points 2 ulps off in every
  lane. The scene's frame for the model (`props::frame`) also got the normals 2 ulps off. Only the ray
  probe at the node's rows matches.
- `./kit check -p game-sim --test untamed_race_quad`: 5 tests pass.

## Not verified / not 1:1

- **The quad does not resolve its hits on the bale.** `Held::refill` refuses a list that names another
  record, so the held list is empty on tick 197 and the gravity resolves nothing. The original resolves
  them against the bale's body. Follow-up MD246.
- **The bale's own frame is not ported**: its pass (its `TripleBalem.col` rays against the quad's
  chassis and the terrain), its resolution and its body step. So tick 198's mirrors and the shove are
  missing, and `untamed_quad_replays_md147_steer` still stops at row 196. Follow-up MD247.
- **The test's Rocksbury world has no bales.** Only this one bale's record is known (from the image);
  the other `TripleBale02` models' records are not. The test hands the one bale to `step_among`. MD247.
- **Hit order when a ray strikes both the world and a body**: the port keeps the nearer point and lists
  hits in ray order. Not seen in any capture (on tick 197 only the bale is struck).
- **The ray tree** is `kiowa::try_ray_tree` over the record's rays, not read from Untamed's record.
  It gives the same 2 contacts in the same order here; untested on other rays.
- **The body's list is just emptied** before the quad's pass. The original's own clear also resets its
  flags; that clear is part of MD247's pass.
- **The body's category** is the one `RecordOn` gives every prop. The quad's mask is 0, so it never
  matters here.

## Next

- MD246: the quad's resolution against the bale's body on tick 197. Check it against row 198's quad
  velocity, splitting the bale's mirrors' share (MD247) out by an Ee write watch.
- MD247: find the bale's update in the frame by an Ee write watch on body `0xaaa6c0`'s velocity
  (`+0x80`). Port its pass, resolution and step, then extend the steer replay to row 204.
