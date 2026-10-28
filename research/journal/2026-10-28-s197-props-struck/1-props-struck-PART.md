# 1 — props struck (PART, blocked)

Task S197. 2026-10-28.

## What I did

No code. I traced what a strike on a prop goes through in the original, decided this task waits on
two others, and blocked it with `--after S198,S195`.

**Why it waits.**

- **S198 builds the struck object.** When the bike's body rays strike a prop's `f.col` mesh, the
  contact's *other* is the prop's **collision object** (the wrapper's `+0x120`; see below). That
  object's `Collider` tuning, its body and its own contact list are the whole of S198 ("a prop's own
  collision object … resolved in the prop's force listener"). S198 had just started in slot s2 when
  this session ran (only an exploration test, `tests/prop_collision.rs`, on its branch). If S197
  built a stand-in collision object now, the two tasks would write the same thing twice in
  `prop_world.rs`/`prop_set.rs` and the merge would conflict.
- **S195 has the proof.** The task says to prove it with S195's capture: a free09 prop struck by the
  bike, live in PCSX2. S195 is still open, far down the plan (about 80th), and this task must not do
  S195's work.

## What the original does

(Recipes: `./kit fn <addr>`. The listed addresses are in research/oracle.txt's S131 block or below.)

- **The f.col mesh's owner is the prop's collision object.** The physics object's init
  (`./kit fn 0x001597b0`) loads `<model>f.col` (kind 5) into `+0x128`. It ORs the flags at
  0x00468f50 and 0x00468f58 into the mesh's `+0x84`, then sets the wrapper's `+0x48` interface as
  the mesh's struck listener (vt `+0x80`). Next it builds the collision object `+0x120`
  (`0x0026f980`, 0x200 bytes) and binds it (`./kit fn 0x0026fcb0`): `+0xf4` = the body (`+0x110`),
  `+0xf8` = the node, `+0x1f0` = the m.col ray probe, `+0x1f8` = the f.col mesh, `+0x1f4` = the sweep
  probe, mass/1/m from the body's `+0xc`/`+0x10`, and the inverse inertia quad from body
  `+0x110..+0x12c`. It also writes **the f.col mesh's `+0x80` = the collision object**. That
  `+0x80` is what `contact_search::Struck::other` reads, so a bike ray that strikes a prop gets
  `other = Some(prop collision object)`, and `contact_search::collide` gives the prop's object a
  kind-1 contact back with the normal turned round. Then the mesh is put in the object's
  ignore-list against its own m.col probe and sweep probe (`0x0011add0`/`0x0011ade0`).
- **The struck listener** (`./kit dis 0x0015ad40 --len 0x40`; `this` is the wrapper after the
  interface thunk): if `+0x58` (CollisionEnabled) is set, it calls the wrapper's vt `+0x124`
  (`./kit fn 0x0015b790`), which wakes **that object**:
  - it does nothing when the body's word 0 (enabled) is already set;
  - otherwise it resets four sub-objects under the collection (`+0x138`'s `+0x50`, `+0x68`, `+0x98`,
    `+0xb0`: the body's force/preforce/post listener slots), enables the body (body `+0x100`'s vt
    `+0xc`, 1), and puts the wrapper's `+0x50` listener on the node (`+0x134`) when `+0x5c` is set;
  - with `+0x54` (it has a collision file) and a collision object, it calls `0x0026f680` on the node;
  - with `+0x6c` (DetectCollisionsWhenDisabled) and an m.col probe, it re-enables the probe
    (`0x0011b070(probe, 1)`, `0x0011b1b0`);
  - then it calls vt `+0x110` (`./kit fn 0x0015b210`: re-files the m.col/f.col/sweep collidables
    when `+0x54` is set), sets `+0x1b0 = 1` and ORs 2 into `+0x28`.

  The listener wakes the struck object itself. It does **not** activate the collection directly
  (`PropSet::wake` does both). Where the activation comes from on a strike was not traced.
- **The push.** The listener itself applies no force. The striker's push is the bike's own resolve
  (`contact::resolve_ridden`): on a hard contact (closing faster than `HARD`) the impulse is shared
  with the other's body (`impulse_on`'s `Side::Body` branch, already ported). The spring applies only
  to the own side. So a soft push into the prop can only come from the prop's object resolving its
  kind-1 contact in its own force listener, which is S198's.

## How it was proved

Nothing was ported, so there is nothing to prove yet. The traces above were read from the
decompilation and the disassembly. None of them was run in the interpreter.

## Not verified / not 1:1

- **Everything in the task is undone**: f.col collidables at the body's node pose; the bike, rider,
  wreck bike and plane searches against them; the wake on strike; both pushes. Reason: it waits on
  S198 (the collision object these contacts point at) and S195 (the capture that proves it).
- What `0x0026f680` does to the node, and what activates the collection on a strike (the listener
  only enables the object), was not traced.
- None of the traces above was checked in the interpreter or on a capture.

## Next

When S198 and S195 have landed:

1. Read S198's prop collision object (its `Collider`, its record and where its list is cleared and
   resolved in the tick). Then decide the order: the bike's contacts gather once a tick, before the
   bike steps (`rider_contact::gather`). The prop's object clears its list in its own collide pass.
   If that clear runs after the bike's search, the kind-1 contact is lost, so check the original's
   order.
2. File each prop's f.col mesh (posed at its body's node) in a new module shaped like
   `mover_contact.rs` (the train cars), but with `Struck::other = Some(prop object)` instead of a
   mover. Give the bike's resolve an `Other { collider: prop's, side: Side::Body(prop body) }` per
   struck prop. Do the same for the rider, the wreck bike and the plane searches.
3. The struck listener: wake the object as described above, then the collection (`PropSet::wake`),
   in the original's order.
4. Replay S195's knock against it.
