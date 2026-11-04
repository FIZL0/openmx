# 1 — the joust parts on the plane's ray (FINAL)

Task R31. 2026-11-03. Research only; no product change.

## What I did

- **Saved images 137859..137866.** I used kit copy 1, slots 65–72, with `research/c10/play_at.sh` on
  `context/recordings/c16/fly.p2m2`. NominalScalar was set to 0.05 for the run and set back to 1.0
  afterwards. The saves were converted with `research/s272/rams.py`, pointed at copy 1's savestates
  (a scratch copy of the script). Each save holds the tick asked for. They are in
  `context/states/s272/` (git-ignored). Copy 1's slots 60–64 still hold F30's 137854..137858.
- **`tests/c16_fly.rs`:**
  - `Tick::plane_body`: the plane's whole body after the tick;
  - `body_diff`: a body against the original's, field by field;
  - `r31_the_joust_against_images_to_137866` (ignored): the probe. After each tick 137854..137865 it
    compares the plane and the wreck bike with the next image, and both contact lists entry by entry.
    It also names the plane rays that end at x = -7.5, the wreck part's mesh collidable, and the disc's
    two bike meshes' root boxes.
- `research/r31/index.md`: the findings. `research/f30/index.md`: a pointer to them.
- Filed F35 (the fix).
- opentennis has no planes or wreck bikes, so nothing was taken from it.

## What I found

- **Through tick 137862 everything matches bit for bit:** both bodies on every field, and both lists.
- **On 137863 the original's plane files a kind-0 entry of its own on the wreck bike.** It is at the end
  of plane ray 33, (-7.5, 2.3962276, -1.6266332), with normal (0.7411895, 0.011839539, 0.6711917). The
  kind-1 mirror goes at the head of the wreck bike's list. After it come the wreck bike's own three
  entries, which equal the port's, and that strike's mirror on the plane.
- **What the ray strikes:** the wreck part's mesh collidable. Its record is the wreck part's collision
  object, with category 0x9a058 (`wreck_bike::COLLIDABLE`) and flags 0x94. It is placed on the wreck
  node, and its root box is `wreckbikef.col`'s, which is the same box as `bikef.col`'s.
- **Why the port misses it:** `flight_search::Bikes` drops a wrecked rider (`in_world`), and nothing else
  puts the wreck hull in the plane's search. `wreck_plane::search_downed` only runs the wreck bike's rays
  on the plane's hull.

## How it was checked

- The probe's printout against the images: `./kit check -- -p game-sim --test c16_fly -- r31_ --ignored`,
  then the test binary with `--nocapture`.
- `./kit check -- -p game-sim --test c16_fly`: the c16_fly tests still pass with the new `Tick` field.

## Not verified / not 1:1

- **Which collidable listener runs on the wreck mesh when the plane strikes it.** The riding bike's
  struck listener (S95) is known. The wreck mesh's listener list (+0xf4) was not read. F35 should read
  it.
- **Whether the plane's sweep probe can strike the wreck hull too.** Only the ray's entry shows here.
- **The order in which the plane's ray record keeps a wreck-hull contact against a terrain or scene one
  on the same ray** was not checked. No ray here has both.
- **How the image's mesh collidable gets its +0x120 matrix.** It equals the wreck node's rows in image
  137864. I did not trace when it is copied in the tick.
