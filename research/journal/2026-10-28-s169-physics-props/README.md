# S169 — physics props

2026-10-28. Task: `./kit plan S169`.

The task asked what the original's "HasPhysics props" are and what their collidables name at
`+0x7c`/`+0x80`, and to port them. There are two kinds. The `.toy`'s `HasPhysics` key appears only on
cars. Those are S127's path-car subclass, and they already give a `Moving`. The other rigid-body props
are the `[PhysicsCollections]` props (bales, crates, barrels, signs, balls), which S80–S198 already
simulate. A collection prop's `f.col` names no mover. It names its owner, the prop's collision
object. So a prop's "Moving" is the owner branch on that object's body. That is now ported as
`Collection::under` / `PropSet::under`, and it matches the original bit for bit on all 62 free09 props
over 10,000 generated wheels. Nothing in a ride stands on a prop yet: the `f.col` is not searched
(S197), and the wheel search is not wired to it (S207, new). Details in 1-physics-props.
