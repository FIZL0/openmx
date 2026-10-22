# 1 — bike collidables (FINAL)

Task S86. 2026-10-22.

## What I did

- New `crates/game-sim/src/bike_col.rs`:
  - `BikeCol::load(m.col, f.col)` builds the body rays through `flight_search::Rays::of` and the frame mesh through `track::Mesh` + `flight_search::facets`. Both reaches come from the files.
  - `probe()` / `mesh()` / `strike()` wrap S46's `ray_probe::pair` with `Shape::Mesh`.
  - `collidables()` gives the four collidables (frame, body rays, second rays, sweep) with every mask, flag and list as the ride leaves them.
  - `ghost()` is the ghost switch's add/remove on a category list.
  - The named masks are `RIDER`, `VEHICLE`, `RIDE_START`, `SECOND_IGNORES`, `ACTOR`, `SHAPE` and `RECORD_ACTORS`.
- New `crates/game-sim/tests/bike_col.rs` with two tests (below).
- `crates/game-sim/src/contact.rs` is not touched. Nothing in it changes: the narrow phase a bike's rays run against another bike is S46's ray-vs-mesh pair, not a new one (see below). Wiring the second rider into the ride's contact search is the rider-against-rider task's job, not this one's.
- research/oracle.txt has an S86 block.

## What the original does

The brief's premise needs a correction: the bike's `f.col` is **not** a "kind 5" collidable.

- **The loader.** `./kit fn 0x00252ff0` loads `f.col` through `0x001ed1e0` → `0x001edda0` into an ordinary 0x250-byte mesh collidable (`+0x68` = 0, constructor `0x001ecf50`). The 5 S78 saw is the second argument of `0x00119a40`, which sets `+0x116` bits: 5 is ENABLED (struck).
  - In all three S93 images, P1's and P2's `+0x7c0` read `+0x68` = 0. The narrow-phase table's case 5 (`./kit fn 0x00109ee0`, from `0x001ebe70`) is a ray tree against a *sphere*. oracle.txt has its node test and its leaf. No bike collidable has that kind, so it is not on the bike's path.
- **Which files.** The file names format bike `+0x28c`, an inline string that reads `bike` for both split-screen players. So P2 (another bike) and P1 (pro rider) both load `Bike\Col\bikem.col` and `Bike\Col\bikef.col`, out of `Tracks\free09_ss_2.res`. S93 found the same for the `.phy`.
- **The masks.**
  - `0x00252ff0` sets the categories from the bike's `+0x390` (0x2000), its `+0x394` (0x80000) and the globals `[0x468f50]` = 8 (rider) and `[0x468f68]` = 0x40 (vehicle).
  - `./kit fn 0x0027aa70`, the bike builder, ORs `[0x4e2128]` | `[0x4e2130]` (0x8000 | 0x10000) into the frame, both ray probes and the wreck part.
  - Result: frame 0x9a048, body rays 0x18008, second rays 0x18000, sweep 8. Collides-with is all ones on all four; the record's actors word is 0x2008.
  - The wreck part, for comparison: collidable 0x9a058, object actors 8, collides-with 0x40000, which are S69's constants unchanged.
- **Ignore lists.** The three others are mode-1 object lists. The second ray probe also has the category list [0x100, 0x2000].
  - The ghost switch `./kit fn 0x00250580` adds the vehicle word to the body rays' and the sweep's category lists (`0x0011b080`: return if listed, else the first 0 slot, else grow). It takes the word out by zeroing its slot (`0x0011b160`).
  - So a bike that was ever a ghost carries [0], and one that never was carries []. Both ignore nothing.

## How it was proved

Both tests are in `crates/game-sim/tests/bike_col.rs` and run on S93's RAM images `splitscreen`, `s93-short-5997` and `s93-long-3402` (copied from s4's `context/states` into `context/states/s93`).

- `both_bikes_load_the_disc_files` checks 6 bikes: each image's P1 and P2. For each it checks:
  - the model name;
  - the rays' root, tree and eleven segments, against the disc file;
  - the frame's tree, triangles (packed normals included) and vertices, against the disc file;
  - both reaches (`+0x60`), bit for bit;
  - each of the four collidables' class, flags (the search bit dropped), nearby bit, category, collides-with and all three lists, against `collidables()` (4 lists replayed through `ghost()` on/off);
  - the record's actors words, the globals, and the wreck part's three words.
- `body_rays_strike_the_other_frame_as_the_original` runs 10,000 cases of P2's body rays against P1's frame:
  - the original's `ray_probe_pair` (vt +0x94) against `BikeCol::strike`, with `ray_probe_clear` first in 1 case in 4;
  - each case runs a segment through a point on a frame triangle, or near one in 1 case in 8, with a random turn and a random record state;
  - result: 7317 pairs struck, 1051 of them on a second pairing, and 20448 contacts written, all bit for bit (return value, probe flags, record and every contact).

## Not verified / not 1:1

- **The kind-5 narrow phase (`0x00109ee0`) is not ported.** No bike collidable is kind 5, so the brief's "ray-vs-kind-5" is S78's misreading of a flag argument. Porting it would ship code that no ride reaches. Its behaviour is written down in oracle.txt in case a sphere collidable turns up.
- **The second ray probe's and the sweep's segments and reaches are not modelled.** They have no file; the ride sets them as it goes. `collidables()` leaves their radius at 0, and the test does not compare it.
- **`+0x31`'s upper bits are not modelled.** Only bit 0 (nearby) is. The upper bits differ between players and images (0x03 / 0x33 / 0x43) and are not the search's.
- **Listeners and callbacks (`+0xf4`, `+0x10c`, `+0x110`) are not read or compared.**
- **The frame's transform is taken from P1's RAM in the oracle test, not built from the bike's frame.** Where the frame sits relative to the bike body is the ride's job (S78).
- **The ghost switch's other effects are not ported:** `+0x7dc`, `+0xb7c` and the model's alpha. Only its category-list change is, as `ghost()`.

## Next

Nothing for S86. The rider-against-rider contact (S78's follow-on) can now call `bike_col::collidables` and `BikeCol::strike` for a second bike.
