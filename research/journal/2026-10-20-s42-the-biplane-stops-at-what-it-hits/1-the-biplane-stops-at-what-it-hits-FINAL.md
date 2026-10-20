# 1 — the biplane stops at what it hits (FINAL)

Task S42. 2026-10-20.

## What I did

- Ported the collision object's contact resolution as `crates/game-sim/src/contact.rs`, a new module.
  `contact::resolve` is what `biplane::collision_force` takes as its closure. It runs per contact:
  relative velocity, then impulse or spring, then friction, then damping. `biplane.rs` is unchanged:
  it still takes the resolution as a closure, and wiring `resolve` into the ride loop is for the
  step that owns the plane's collision object. That object is filled by S43.
- The other side of a contact is a `Side`: a `Rigid` body, or a vehicle's chassis with its
  transform (`Vehicle`, `Node`). A mover (`Mover`) has a spin and a velocity. `Weighed` wraps
  `biplane::Body` with its mass as a `Rigid`.
- The proof is `crates/game-sim/tests/contact.rs`. It runs the plane's own collision object in the
  original over 10 000 generated cases.
- Named the callees in `research/oracle.txt` (S42 block under `collision_resolve`). Updated
  `collision_resolve` to "ported".
- Filed **S43**, the contact search that fills the list (see "The brief's premise" below).

## What the original does

`./kit fn 270dc0` (record, impulses). For each of the record's `+0x100` entries (`+0x108`, 0x80
bytes each):

1. **Relative velocity** (`./kit fn 270f60`). The record's centre of mass `+0x90` goes into the
   world (`+0xa0`), turned by the body's quaternion. The contact point goes back into the body's
   frame relative to it, giving the entry's `+0x50` arm. The point's velocity is the rates crossed
   with the arm, turned back, plus the body's velocity.
   - The other side is the same for an other record on a body (`+0xf4`).
   - On a chassis (`+0xf0`, transform `+0xf8`), it goes through the transform's rotate/unplace
     (`./kit fn 1b2ff0`, `./kit fn 1b3180`), and the chassis' `+0x190`, `+0x260` and `+0x290` are
     read.
   - For a mover (`+0x78`), it uses its vt `+0xbc` spin and `+0xb8` velocity, its transform `+0x254`
     and its origin `+0x190`.
   - The difference goes to `+0x1e0`, and its dot with the normal to `+0x1d8`.
2. If that is under −10:
   - with the impulses argument set and the entry of kind 0, it runs the **impulse**
     (`./kit fn 2719b0`), a textbook one with the restitution and the inertia terms;
   - otherwise it does nothing.

   If it is not under −10, it runs the **spring** (`./kit fn 271590`):
   `max(k·min(depth, +0x118) − d·speed, 0)`. The anchor is on this body for kind 0, and on the
   other's for kind 1, with the depth negated.
3. **Friction** (`./kit fn 2722d0`) always runs: `32.2 · mass · min(0.1·|slide|, 1) · mu` against
   the slide. Mu is the actor's (`+0x194`), the scenery default (`+0x190`, surface −1), or the
   surface table `+0x198[surface]`.
4. **Damping** (`./kit fn 2724c0`) runs when `+0x50` is set and the record is on a body (`+0xf4`). Each lane of the body's linear damping is
   lowered toward `+0x58` by how much the normal lies along that axis. The angular damping is lowered
   toward `+0x60` along the contact's moment axis. Both are stored through vt `+0x1c`/`+0x54` as whole
   quads.

Spring and friction forces go through `./kit fn 272880`: the masked moment through vt `+0x5c`, then
the force through vt `+0x64`. Two records whose collidables each collide with the other share the
spring and the impulse by the records' masses (`+0x64`/`+0x68`, inertia `+0x80`). Otherwise the
bodies' own masses are used. A collidable in the actors mask (`+0x110`), or a mover, selects the
actor tuning (`+0x13c…`) over the scenery's (`+0x124…`).

**The transform's transpose is whole.** `./kit fn 1b30e0` builds the columns with MMI ops. The third
column is a 64-bit `move` over the third row, which keeps the row's `z`/`w`. I first read it as a
128-bit copy, which would put `r0.w` in `z`. The oracle disagreed (chassis/mover other arm `z`), and
the whole transpose matches.

**The damping's `w` lanes** come from the stack: the friction call just before left the normal's
`w` there. The port writes `normal.w`. The oracle agrees in every damped case, and a mutation
(the old `w` kept) fails at case 0.

### The brief's premise

The resolution is not where the contacts, the record's `+0x40` contact pointer, `+0x44` or the
`+0x1fc` surface come from. `collision_record_collide` (`./kit fn 2708a0`, vt `+0x7c`) writes all of
them, before the step. The resolution only reads the list. Filed as S43.

### Water and the tire's held contact (`+0x84`)

Neither is reached by the resolution. Its only calls are its five parts, the transform helpers, the
body's vtable entries and the mover's two virtuals (listed with `./kit dis` on each). None of them
touches a tire or anything water-related. The contact search (S43) calls the actor and terrain
searches (`./kit fn 11a9d0`, `./kit fn 11a150`) and `./kit fn 1dce80`, which is not named yet. That
is the last place water could come in, so it is in S43's brief. Nothing in S42's reach stores to a
tire's `+0x84`.

## How it was proved

Function oracle, `crates/game-sim/tests/contact.rs`, `the_contact_resolution_matches_the_original`:
10 000 cases on C7's biplane save state.

- The plane's real collision object is used, through its own vtable slot (the test asserts it is
  `collision_resolve`), on the plane's real body with that body's vtable. Nothing in the original
  is hooked except the mover's two virtuals.
- Each case generates:
  - the record's tuning;
  - the body;
  - 0–4 contacts (normals unit or not, `w` anything, surfaces −1…15, both kinds);
  - two other records in scratch, each on a copy of the plane's body or on a chassis with a clean
    transform;
  - a mover;
  - the impulses flag.
- It compares, bit for bit:
  - `+0xa0`, `+0x1d8` and `+0x1e0`;
  - every entry's two arms;
  - every field of the plane's body;
  - the others' `+0xa0`;
  - the other bodies, every field;
  - the chassis' velocity, rates, shock and twist.

What the cases reached, counted on the one-contact cases and asserted ≥ 100 each:
16 995 contacts, 2 116 hard, 1 315 impulses (296 on a chassis), 2 837 springs, 1 499 mutual,
1 715 actor, 3 890 with a chassis, 7 490 with a body, 3 726 with a mover, 2 855 of the other's kind,
6 737 damped cases.

Mutations: the friction's 0.1 one ulp up fails at case 12 (force), and the damping's `w` from the old
value fails at case 0. Both were restored, and `./kit check` passes in full.

## Not verified / not 1:1

- **Own side on a chassis is not ported.** `./kit fn 270f60` also branches when the record has no
  body (`+0xf4` = 0) to a chassis' transform. The plane is always on its body, so only that side is
  ported and proved.
- **The damping's `w` lanes are stack leftovers.** The port writes the normal's `w`, which is what
  the friction leaves at that stack slot. It is proved only with the friction running just before at
  the same depth, which is always the case in this routine.
- **Contacts of kind 1 without an other** panic in the port (`expect`). The original would read
  through a null record. No such contact was generated, and none should exist.
- **Surface indices outside −1…15** panic in the port, where the original would read past the
  table into `+0x1d8…`. They were not generated.
- **The transform's lazy rebuild** (`./kit fn 1b1750`) is not ported. The port takes `Node` as
  already built, and the test hands it clean (`+0x110` bit 0).
- **The mover's virtuals** are values in the port (`Mover::spin`, `Mover::velocity`), and hooks in
  the test. What a real mover returns is not proved here.
- **Body listeners** (`+0xd0`) are not told: the plane's body has none in the state, and the copies
  have none. `Weighed` adds to the fields directly.
- **Generated ranges are bounded** (positions ±20 around the body, speeds ±30, masses around the
  shipped ones). Infinities and NaNs were not generated.
- **Not wired into the ride.** `biplane::collision_force` still takes the resolution as a closure.
  Nothing fills a `Collider`'s contact list until S43.

## Next

S43: the contact search (`./kit fn 2708a0`) that fills the list, and with it the water question.
