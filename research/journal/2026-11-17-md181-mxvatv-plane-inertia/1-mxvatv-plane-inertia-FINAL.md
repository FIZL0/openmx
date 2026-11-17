# 1 — mxvatv plane inertia (FINAL)

Task MD181. 2026-11-17.

## What I did

- `Flight::new_mxvatv` now builds the body from SLUS-21104's own inertia, one per model
  (`flight.rs` `MXVATV_INERTIA`, `Flight::mxvatv_inertia`). Before, every MX vs. ATV plane used
  MX Unleashed's (C7's `INERTIA`).
  - Model one (skin biplane): (2833.5432, 5880.4233, 3846.8018). This is a few ulps off C7's
    (2833.59, 5880.47, 3846.80), so it is not the same tensor.
  - Model two (skin biplane3): (5199.0327, 11892.645, 7951.833).
- The body's inverse and the kept `inv_inertia` are `inverse_inertia_mxvatv` of it (the body setter).
- The collision object's inverse inertia is scaled from the model's inertia
  (`wreck_bike::collider`, as for MX Unleashed's plane).
- The wreck (MD112's rule) now scales the model's inertia, not C7's. The flight keeps only the
  inverse (that is all a snapshot carries), so `built_inertia` finds the tensor again by matching the
  inverse against the three builds. An unknown inverse falls back to C7's. I chose this over a new
  `Flight` field so the netplay wire and snapshot stay the same size.
- `phy::MxvatvPlane` gained `model_one` (which model the file was read as), set by `read_mxvatv`.
- Nothing taken from ../opentennis: it has no plane.

## What the original does

- MXVehicle's base loader reads `[Mass Properties]` into the body. The body's inertia (body `+0x130`)
  comes from the model's box times `InertiaScale`, as in MX Unleashed. The loader then copies it to the
  vehicle's `+0xcc0` (research/oracle.txt `# MD150`, `# MD112`). The body's `+0x140` is its inverse.
- The two models have different boxes, so different tensors. The idle second plane in both MD80a
  images (gate 0) holds model two's: its form word is 0, which reads as model two.
- The collision record (vehicle `+0xc8`) holds `CollisionWeight`'s mass at `+0x58`, the scaled inertia at
  `+0x60` and its inverse at `+0x70`.

## How it was proved

- A save-state check, `crates/game-sim/tests/mxvatv_biplane.rs`
  `the_mxvatv_biplane_starts_from_its_models_inertia`, on MD80a's biplane.ee.bin (model one) and
  biplane3.ee.bin (model two). The plane is built from `biplane.phy`. Bit for bit:
  - the inertia against `+0xcc0` and body `+0x130`;
  - the body's inverse and the kept inverse against body `+0x140`;
  - the record's mass (`+0x58`) and inverse inertia (`+0x70`).
- The same test throws the plane off the map's edge. The wrecked body's inverse is
  `wreck_inertia_mxvatv(WreckInertia, image +0xcc0)`, bit for bit. The reset chord puts the built
  inverse back.
- MD112's oracle already proved the wreck rule itself on biplane3.

## Not verified / not 1:1

- The inertia is carried from the images, not worked out from the model's box. The box is in the
  model library, which game-sim does not read. This follows C7's `INERTIA` and the Kiowa's
  `KIOWA_INERTIA`. A mod plane with another model would still get model one's or model two's.
- A wrecked plane's inertia was not checked on a live capture or a flown run in PCSX2. It was only
  checked against the build in the save states and the wreck rule MD112 proved.
- Skins with other form words were not checked. They map to model one or two by `mxvatv_model_one`,
  and only biplane (form 1) and biplane3 (form 3) have images.
- `built_inertia` recovers the tensor from the inverse. A flight seeded from other RAM (a replay
  test that writes the body's inverse) wrecks with C7's inertia unless its inverse matches one of the
  builds. Retail builds always match.
- Flights made as `Flight { mxvatv: true, ..Flight::new(..) }` (tests/snapshot.rs, openmx's lobby and
  netplay tests) still carry C7's inertia. They do not go through `new_mxvatv`.

## Next

Nothing.
