# 1 — gravity and friction zones (FINAL)

Task MD63d. 2026-11-02.

## What I did

- `handling.rs`: removed `FrictionZone1 → [Collisions] TerrainFriction` and `GravityScale →
  [Mass Properties] Gravity` from `mxvatv_ini`'s moves. Added `MassPart` (with `MassPart::read`), which
  holds what the original's reader leaves from those keys. `mxvatv_phy` writes it over MX Unleashed's
  reader's result: `mass.gravity` (the scale), `mv.gravity` (the force), and `collisions.friction`,
  `body_friction` and the new `surface_friction`.
- `phy.rs`: `Collisions::surface_friction: [F; 16]`, the record's per-surface frictions. MX
  Unleashed's reader fills all sixteen with `TerrainFriction`, which is what `rider_contact` used to
  build inline. `rider_contact::collider` now reads it, so MX Unleashed's rides are unchanged.
- `tests/mxvatv_mass.rs` (new): the oracle, plus a no-image test that `mxvatv_phy` carries the values
  and that the stand-ins are gone. `tests/mxvatv_air.rs`' move test now expects no `Gravity` and no
  `TerrainFriction`.
- `research/oracle.txt`: an `# MD63d` block (the reader, the ini lookups, readers and rewriters).
- Filed MD63e: the rest of the part reader against `Phy::read` (mass `Weight × 0.0310559`, defaults).
  Filed MD63f: the wreck and put-back rewrite the gravity.
- opentennis: nothing to take (no MX vs. ATV, no part reader).

## What the original does

`./kit fn 0x00493210 -o atv`. It is called from 0x004a7e70, the vehicle base's read: the
`[Mass Properties]` section, then this, then `[Wrecks]`. The bike loader 0x004def40 calls that and
then re-reads `[Mass Properties]` for Inertia, CofG, WreckCofG, BalanceCofG, the CofG travels and
YawRateLimit.

- `Weight` (default 500) is read first. Then `CofG`, then `GravityScale` (default 1), written straight
  to `*(part+0xe8)+0`. Later in the function, +0x40..+0x4c = `(0, neg.s(scale) × Weight, 0, 1)`
  (`./kit dis`: `neg.s` then `mul.s`). The body gets the mass `Weight × 0.0310559` through vt +0x18.
- `FrictionZone%d` for 1..16, each default 1, then `Friction` (default 1) and `BodyFriction`
  (default the `Friction` just read). The setters: 0x002049b0 (value, record, k) writes
  record+0x1a8+4k for k in 0..16, and 0x002049e0 writes +0x1a0 and +0x1a4, with a −1 body friction
  becoming +0x1a0. The record is part+0xc8. That is MX Unleashed's +0x190/+0x194/+0x198, 0x10 higher.
- Readers: the gravity add 0x00493f60, the first call of `atv_bike_ground_step` (through the thunk
  0x004a4ef0), adds +0x40 to the body by vt +0x80. `atv_bike_stand_upright` reads it too (MD63b).
  The collision friction 0x00206c70 takes μ from +0x1a0 for surface −1 and from +0x1a8 + 4·surface
  otherwise, and from +0x1a4 against an actor. That is MX Unleashed's collision_friction, which the
  port's `contact::friction_on` already reads from `Collider::surfaces`.
- Rewriters (not ported, MD63f): the wreck 0x004d3f90 writes `-WreckGravity × body+0x18 × 32.2`, and
  the put-backs 0x004d40a0 and 0x004d40b0 write `-GravityScale × body+0x18 × 32.2`.

What MX Unleashed's reader did with the stand-ins differed in two ways. The force was
`-(Weight/32.2) × scale × 32.2`: on the first generated file that is −1475.4088 against the
original's −1475.4087. And all sixteen surfaces got zone 1's value. The shipped `mx_*.phy` files set
all sixteen zones equal and `Friction`/`BodyFriction` explicitly, so on those files only the force's
rounding moves.

## How it was proved

Function oracle `mxvatv_mass_properties_gravity_and_frictions_are_the_originals`. It runs
SLUS-21104's reader in ps2-oracle off `context/mxvatv-ps2/state04.ee.bin`, over 10,000 generated
`[Mass Properties]` sections. Each key is present or absent, values are written as files write them
(whole, 1–7 places, signed, 0, 1), BodyFriction is sometimes −1, and CollisionEnabled is sometimes
False. Only the ini's raw string and flag lookups (0x00142ce0, 0x00142ed0) are hooked, along with
the section select, the model's box and the kernel's syscall stubs, which the C library's parse and
print reach (answered 0). So the original's own number parse reads every value. Compared word for
word: GravityScale at +0, the force's four lanes, and the record's Friction, BodyFriction and the
sixteen zones. Every present/absent case of Weight, GravityScale, zones 1 and 16, Friction and
BodyFriction (−1 too) has ≥ 500 files.

Each of these mutations reds it: MX Unleashed's force formula, every zone read from FrictionZone1,
Friction defaulting to 0, no −1 copy, Weight defaulting to 0, and zones defaulting to 0.

`./kit check -- -p game-sim --test mxvatv_mass --test mxvatv_air --test mxvatv_step --test
mxvatv_ride --test phy --test rider_contact --test ride --test hash --test snapshot --test
air_wrecks`: 134 passed. `cargo build -p openmx` builds. `./kit audit`'s only leak is the existing
plan/DONE.md:519.

## Not verified / not 1:1

- **No MX vs. ATV ride capture** (as MD63a–c): the reader is proved on its own, not a ride's gravity
  or friction against a recording.
- The value parse is the original's in the oracle and the port's `phy::number` in the port. They
  agreed on all 10,000 files, but that is generated text, not every form a file could use.
- Only the reader's gravity block and the record's frictions are compared. Everything else it
  writes is still MX Unleashed's vehicle reader under MX vs. ATV's numbers: the body mass
  `Weight × 0.0310559` against `Weight/32.2`, Weight's default 500 for anything but the force, and
  the part defaults for the springs, radius and dampings (MD63e).
- After a wreck, SLUS-21104's gravity is `-scale × mass × 32.2`. The port keeps the load's force
  (MD63f). body+0x18 being the mass is inferred from the rewriters, not checked.
- The tail of the reader (CollisionEnabled off with shapes or a model) is not run: the harness
  gives it no shapes.
