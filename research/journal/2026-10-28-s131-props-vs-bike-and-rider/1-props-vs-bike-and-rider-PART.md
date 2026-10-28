# 1 — props vs bike and rider (PART)

Task S131. 2026-10-28.

## What I did

S131 is "after the props tick", which is S130. When this session started, S130 was still open and
running in slot s2 on the same base commit (594360a5). It had no commits yet, only an untracked
`crates/game-sim/src/prop_world.rs`. Four of S131's five parts build on that world: the prop's f.col
as a *moving* collidable, contacts both ways (the props need a body step to answer an impulse), the
props' state in Sim's hash and rollback snapshot, and the shell drawing props at their body pose. Writing
them now would mean either a second props world that collides with S130's or code built on nothing.
So I did only the part that stands alone, and blocked the rest with `--after S130`.

Done: **the node box computed from the model instead of read from RAM.**

- `props::NodeBox::from_model(psg)` reads the box from the model's `.psg`: node 0's record (centre at
  +192, half extent at +204 of the 224-byte record), with both `w` lanes 1 as the original's node holds
  them. A model with more than one node is an error, not a guess (see Not verified).
- `props::model_file(slt)` gives `<SLT>.psg`.
- `tests/props.rs` now builds every free09 body from the model's box. It also checks each box against
  the state's node, bit for bit and in all four lanes. The old RAM read stays only as that comparison.
- research/oracle.txt S80 block: `psg_node_read` (the node reader) and the instance copy.

Nothing from opentennis applies: it has no scene-prop rigid bodies.

## What the original does

`./kit fn 0x001b3890` reads a `.psg`'s node records: the count, then for each node its name (0x80
bytes), its local matrix (0x40 bytes) into +0x40, the 12-byte centre into +0xc0, the 12-byte half
extent into +0xd0, a word into +0x114, and the parent index. The first node is the scene node itself
(`*param_3 = node`); a further node is made through the vtable's +0x80. Bits 5 and 2 of +0x110 are set
every time, so a loaded model always has a box. In the model loader (`./kit fn 0x001a0360`), an instance
of a model that is already loaded copies +0xc0/+0xd0/+0x114 from the template when the template has no
children; otherwise it clones it through the vtable's +0x90. The bounds query node_bounds (S80) merges
children's boxes (`./kit fn 0x001b42a0`) through their world matrices relative to the root.

## How it was proved

- A save-state capture: free09's 62 bodies in first_recording's state. Each model's `.psg` box equals
  its node's +0xc0/+0xd0 quad bit for bit, all four lanes. The bodies built from those boxes still
  match the original's inertia and everything else S80 checks. Test:
  crates/game-sim/tests/props.rs `free09s_props_are_the_collections_the_original_built`.
- A one-off disc probe (a throwaway test, since deleted) ran `props::claim` on every track's `.toy`
  (free06–11 and their _ss copies, nat01–22, sx01–24). Every knockable body's `.psg` has exactly one
  node and none is missing, so the child merge never runs for a prop.

## Not verified / not 1:1

- The box is checked against RAM on free09 only (62 bodies). The other tracks' props use the same
  reader, but no state exists to compare them against.
- The child-box merge (`./kit fn 0x001b42a0`) is not ported. `from_model` refuses a multi-node model.
  No knockable prop on the disc has one (probe above), so nothing here needs it.
- Not done, waiting on S130: each prop's f.col as a moving collidable in the ride's searches; contacts
  both ways (the prop's impulse response needs S130's collection world); props in Sim's hash and the
  rollback snapshot; the shell drawing props at their body pose. Nothing of these is written.

## Next

Once S130 lands, read its `prop_world.rs` and the props' place in Sim, then:
1. Load each body's `<node>f.col` (props::shape_file) as a kind-5 mesh. Place it each tick at the
   body's pose, keep its previous matrix as S136/S145's Moving does, and add it to the ride's searches
   (the wheel's, the body rays' (rider_contact), the wreck bike's and the ragdoll's) in the original's
   list order.
2. The contact's impulse on the prop's body through S130's world, on the same substep as the bike's.
3. Add the props' bodies (and enabled/reset state) to Sim's hash and the rollback snapshot.
4. In the shell, draw each prop model at its body pose (the node's written-back pose) instead of the
   scene placement.
5. Prove it on a capture of the bike knocking a free09 prop (S130's capture, if it has the bike in it).
