# S274 — the wreck bike against the plane after C16's joust

Journal: `research/journal/2026-11-03-s274-wreck-bike-meets-the-plane`. This is the RAM side.

- **Wreck part:** `*(rider + 0x7a8)`. Its `+0x110` is the wreck bike's body and its `+0x120` the wreck
  bike's collision object.
  - At s272-137910.ram, P2's rider is 0xdee010, its part 0xdc2c50, the body 0xdc2e40 and the object
    0xdc4550.
- **Collision object:** mass at 0x64, inv_mass 0x68, cofg_world 0xa0, the touching flags 0x40..0x4c,
  the sweep at 0xd0/0xe0, the node at 0xf8 (rows +0x80, origin +0xb0), normal_speed 0x1d8,
  relative 0x1e0.
- **Masses:** the wreck collider is 3.1055897e-6 (inv 322000.03); the plane collider 310.559 (inv 0.00322).
- **Lists at 137880 and 137910:**
  - the wreck bike's: terrain, terrain, then a kind-0 entry naming the plane's object;
  - the plane's: one kind-1 mirror naming the wreck bike's object.

  The mirror at 137910: point (4567.279, 139.29706, 3515.2068), normal (0.3042866, 0.95253515,
  0.009285104).
- **P2's wreck state:** wrecked (reason 7) from row 137857 to 138012, settled from 137913, wrecked
  again (state 1) at 138191.
