# 1 — the riding fade's snapshot (FINAL)

Task F12. 2026-11-02.

## What changed

- `game_render::anim::Set::rewind_drawn(snapshot, drawn)`. `drawn` is one model-space matrix per
  joint. Each joint's local is `M_j · M_p⁻¹` (the parent's inverse is taken as rigid). Its rotation
  goes through `mat4_to_quat` and is normalised. Its translation is the local's row 3. `under` and
  the simulation's `Snapshot` are both taken from that. A tree of the wrong length gives `Set::rewind`.
- `game_render::rider::Body::drawn: Option<Vec<M4>>`. When it is `Some`, `Body::posed_by` rewinds
  from it on a restart or a put. When it is `None`, as before.
- `openmx` (main.rs, the offline tick, after `body.ride`): `body.drawn` is set to
  `body.ragdoll_palette(rigged, riding_bones, chassis_at(ride))`. That is the palette `Shown::of`
  draws this tick, so the next tick's restart fades from what was on screen, as the original's does
  from the nodes the row before left. It is `None` whenever `riding_bones` is: the riding ragdoll off
  (`OPENMX_RIDING_RAGDOLL=0`), not placed yet, or the rider down. Online, `posed_by` is called from
  online.rs, which never sets the field, so nothing changes there. The riding ragdoll is not stepped
  online anyway.
- The brief says the shell's riding ragdoll drawing "stays off by default". That ruling was lifted
  on 2026-11-02, and S90 turned it on (`riding_ragdoll_on`). This task does not change the default.
  It only reads the bones when they are what is drawn.

## Translations: not as V109 expected

V109 saw the original's translation buffer (+0x24c) left alone at riding restarts. It concluded that
the riding fades never read translations. In the port they do. With rotations only off the nodes,
and translations from `skeleton.nodes(pose)`, V109's replay fails: P1 at 140378 (33) is 0.126 off.
With the drawn tree's translations as well, all 14 + 50 fading ticks are within 1e-4. That second
version is what `rewind_drawn` does. Why the original's buffer holds still while its pose agrees
with the nodes' translations was not traced.

## How it was checked

- `the_2p_fades_start_from_the_ragdolls_nodes` (game-sim `tests/rider_anim.rs`) now seeds through
  the product path. `Seeds::nodes` composes the original's node tree of the row before into world
  matrices (`Skeleton::world`) and hands it to `Body::drawn`. `posed_by` then takes the snapshot
  itself through `rewind_drawn`; before, the test wrote `Set::under` and skipped the rewind. It holds
  P1 14 and P2 50 fading ticks within 1e-4, the same as before. The port's own snapshot stays
  0.21 / 0.74 out.
- An earlier version left the quaternions unnormalised. Composing and then decomposing the tree
  left them up to 5e-4 short of unit on the deepest joints (j11, j15), so they are normalised.
- New `a_drawn_rewind_takes_the_drawn_nodes` (game-render `tests/anim.rs`): a three-joint chain
  posed one way and drawn another. The rewind comes back with the drawn locals (rotation within
  1e-6, translation within 1e-5), and a wrong-sized tree gives `Set::rewind`.
- `cargo test -p game-sim --test rider_anim`: 17 passed. `./kit check -- -p game-render`: 293
  passed. `./kit check -- -p openmx --bin openmx --test flip_tricks --test foot_down --test ride
  --test wreck_camera --test original_mode`: 382 passed.

## Not verified / not 1:1

- **The shell was not watched.** A fade's starting pose now follows the ragdoll's bones, and the
  drawn rider is the ragdoll pulled toward that pose. Nothing measured how this changes the drawn
  sway through a trick entry or a landing. It was not compared against PCSX2 either.
- **No test runs the port's own riding ragdoll in the 2p replay.** The proof feeds the original's
  nodes. So the 1e-4 depends on the port's riding ragdoll being the original's, which S90/S201 hold
  to ~0.04 ft, not bit for bit.
- **The timing pairing is inferred.** The port draws tick N as pose N plus the ragdoll after sim
  tick N, which steps toward pose N-1. The snapshot at N+1 takes that drawn tree, read as "the nodes
  as the row before left them". The port's ragdoll phase was not proved to be the original's phase.
- The rigid inverse assumes the drawn parents are rotation + translation. The ragdoll's nodes are
  (normalised quaternions). The clip-placed ones are only as rigid as the `.psg` locals.
- The wreck's ragdoll (the rider down) is not used as a source. A restart while down falls back on
  the clip pose. V109's wreck-entry restart (140365) is taken while still riding, so it is covered.
- P1's early reverse restart (F13, ~4e-2) is separate and untouched.
