# 1 — nat05 plant spheres

Task F69. 2026-11-17.

## What I did

- `atv_props`: `solid_sphere` builds an esb sphere solid (`eco::SPHERE`, kind 2) as its part;
  `plant_spheres` does each species (and its four scale words); `PropWorld::spheres` holds them, the
  unit the plants' cell.
- `atv_props::Rays`: `part` stands the species' sphere at the plant (`atv_plant::stand`, F54) and keeps
  it; `pair` on that part sets the record's first to 1 and runs `atv_part::sphere_reaches` (hit bit, no
  contact), as for a sphere zone. Non-sphere parts still go to the quad's `Wheels`.
- `tests/mxvatv_bike_props.rs`: the hit-bit allowance is gone (`assert_eq!` on every frame); new test
  `the_plants_spheres_are_the_images`.
- Nothing from `../opentennis`: it has no plant or sphere collision.

## What the original does

- Each species' parts (`+0x19c`) on state 4 are kind 5 spheres (vtable `0x3b6ec0`, category 0x80), one
  per esb solid: point `+0x130` = the solid's `at` with `w` 1, frame rows `+0x140..` the identity,
  radius `+0x180` the solid's, `+0x184` its square. All 10 Nat05 species' solids are esb kind 2.
- The ray probe's sphere pair reads the sphere's centre `+0x50` (the refresh's frame × point), `+0x180`
  and `+0x184` only (F46d's oracle notes). The stand's scales (`+0x188`, `+0x18c`) are not read there.

## How it was proved

Interpreter oracle on state 4 (`context/mxvatv-ps2/state04.ee.bin`), `tests/mxvatv_bike_props.rs`:

- `the_plants_spheres_are_the_images`: every species' scale words, every part's point, frame rows,
  radius and radius² bit for bit against the image; the eco's unit equals the port's cell; no plant
  entry in the tree has the hidden or turned flag.
- `the_bikes_probes_search_nat05_as_the_original`: 1459 frames a probe; list, record and now the hit
  bit equal on all of them. Struck with no contact (a plant sphere or a zone alone): 9 / 13 / 8 frames
  (trigger / wreck / riding); F34 had to excuse 9 / 11 / 7 of them.

## Not verified / not 1:1

- The stand's two scales (`+0x188`, `+0x18c`) are worked out and thrown away: the ray pair does not read
  them and nothing in this search does. Their values are F54's proof, not compared here.
- Hidden and turned plants: the port's plant groups carry no flags, so the stand is given none. State 4
  has none (asserted); a map with hidden or turned plants would differ.
- Part kinds other than an esb sphere (cylinders, meshes) still go through the quad's `Wheels`
  unchanged; Nat05 has none, so no frame here tries them.
- The sphere's frame row 3 `w` and the point's `w` are taken as 1 (the identity and the builder's);
  matched on the image's parts, not on the original's constructor run.

## Next

none
