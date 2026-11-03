# MD75d — the spawned quad read out of the image

2026-11-03. Task: `./kit plan MD75d`.

`tests/atv_quad/image.rs` (mounted in `tests/atv_quad.rs` as `mod image`) adds `read_ride(ee, quad, dt)`.
It reads a `Ride` (the test harness's: `State` with `Params`, `Quad`, `Body`, the contacts and the
balance words, plus the `quad::Bike` with wheels, more, spins, parts, engine, air keys and the callee
answers) from MD37a's `atv-r-spawn.ee.bin`. The quad is read at its own address, 0x015e0690, and every
other part through the quad's own pointers. It returns an `Image` beside the ride, which records where
each part was. It also lists `Image::gaps`: the words the ride cannot carry whole, the bits it does carry,
and why.

The reader is `poke_ride` run backwards: every word the poke lays is read from the same offset. Float
words are carried bit for bit (`F(f32::from_bits(..))`), not as the FPU would read them, because a
denormal sitting past the engine's curve points must survive the round trip. `F::from_bits` flushes it.
Words that `poke_ride` never writes are copied the way `Gen::ride` copies them, from the same words
(the bike wheels' steering, contact and spring, `Spin`'s brake/donut/rear-pair/spin/torque/`1/r`, and
`More::chassis_velocity_local`). That keeps the ride the same shape the generator makes. `quad::Steps`
copies them in again anyway.

Nothing was taken from opentennis: it has no RAM-image reader of a vehicle.

## The check

`the_spawned_quad_reads_back_word_for_word`: read the ride, then poke it with `poke_ride` into a second
copy of the image at the harness's layout (OBJ, BODY, WHEEL, ENGINE and so on, regions zeroed and the
written set cleared). Every word the poke wrote is then compared with the image's word at the
corresponding place. Each harness block maps to the image's block (quad, body, world, state, record,
ratios, the two curves, engine, gate, node, four wheels, the scene nodes, the box). A word counts as
equal if it is equal, or if it is a pointer to the same place in the other layout, or if it is a
pointer to one of the harness's stand-ins: `WORLD` for the wheels' ground, the hook vtables for the
record's and the nodes', `ENTRY`, `collidable_at(..)`, `ACTORS`, `SEARCH`. A word inside a declared gap
counts as equal if it matches on the bits the gap carries. The harness's own hook tables are skipped,
and so is the struck thing when the image has none.

Result: **2,654 words compared, all equal**, with 13 gaps declared. Then two words of the read ride are
changed: one bit of `lean`, and a contact-frame row of wheel 3. The comparison must go red at exactly
those five image words, and it does.

Mutations of the reader, run by hand, 5 in total. Three went red: rpm read from +0xa4, a wheel's
`max_damper_speed` read from +0x44, and the +0x14ec gap widened to the whole word. Two stayed green, and
both are equivalent on this image. `lean_fore_aft` read from `lean`'s word passes because both are 0 at
the spawn. Leaving the parent links out of the scene walk passes because node 0 has no parent, so the
same four nodes are kept.

`./kit check -- -p game-sim --test atv_quad -- spawned_quad_reads_back`: TESTS PASSED: 1 tests.

## The gaps (what MD75e starts from)

- **The scene.** The quad's node has one child, and below that is the rider's skeleton: at least 40
  nodes, 0x130 apart. `atv_moto::Scene` holds `NODES` = 4, so the reader keeps the first four
  breadth-first (node 0, its child, the child's first child, that node's first child). Two links lead
  past them (node 2's sibling, node 3's sibling) and are declared as gaps. MD75e's brief already lists
  "scenes past atv_moto::NODES nodes".
- Each kept node's `+0x100` word: the ride carries the two flag bytes, and the other two bytes are not
  the ride's.
- Quad `+0x44`: the ride carries the collision sync's bit 0 and the `+0x45` flag byte. The image has
  0x67 in the `+0x44` byte, and only bit 0 is modelled.
- Quad `+0x14ec`: only the low byte, as the buttons nibble plus `Ride::high`. On the quad the word is
  0x636f6c00, which is text (oracle # MD37a already says it is not the buttons).
- Each wheel's `+0x110`: the contact record's first byte. The harness fills the rest with 0x5a.
- Record `+0x20c`: the image has 0, meaning nothing is struck, but the harness always lays a thing.
  `Ride::class` is 0 for it.

## Decisions

- **The reader is test code, beside the harness.** It needs `Ride`, `poke_ride` and the layout
  constants, which are private to `tests/atv_quad.rs`, and the product may not hold image addresses. A
  file of its own keeps the merge with the other atv_quad tasks to one line.
- **Pointers are checked by place, not by value.** The harness lays its own blocks, so a pointer the
  poke writes counts as equal when it points to the same offset of the block standing for the image's.
  The image's wheel ground and the world's box are the same object (0x0184c7d0), while the harness's
  ground is `WORLD`. The stand-in list allows exactly that.
- **`dt` is an argument.** It is not a word of the quad's. The frame's step dt is MD75e's to pick.
- **Only still ground is read.** A wheel whose `+0x1f0` or `+0x1f4` is set is an error from the reader,
  not a guess. At the spawn all four are 0.

## Not verified / not 1:1

- Only this one image was read (the spawn). `atv-r-air`, `atv-s-*` and `atv-r-spawn-late` were not
  tried. The `atv_s_` quad sits at 0x015e0b90 and would need that address.
- The scene past four nodes is not carried (see the gaps). A frame run from this ride will not update
  the skeleton's world frames as the original does. That is MD75e's.
- A wheel standing on a collision object or a mover (`atv_wheel::Ground::Object`/`Mover`) is not read.
- Words that `poke_ride` does not lay are not read either, because the round trip cannot check them:
  the air keys outside the preload/backflip/airborne set, `Held`'s levers (which `Steps` copies from
  `Quad`), the player and tail, `Quad::node_local` (a call's answer, set to (0, 0, 0, 1) as the
  generator does), and the ground query's answers (none, and `echo` off).
- The tables are read whole, past `gears` (5 of 8 ratios) and the curve's 10 points (of 16). The words
  past the counts are whatever the image has there, carried unchanged.
- The two mutations that stayed green are equivalent only on this image's values. They are not
  equivalent in general.
