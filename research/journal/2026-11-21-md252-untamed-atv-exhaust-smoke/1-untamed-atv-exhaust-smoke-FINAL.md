# 1 — untamed atv exhaust smoke (FINAL)

Task MD252. 2026-11-21.

## What I did

- **Found the node.** Of the KTM 250 build's five parts (`ATV_R_250_KTM.sub.bpb`: `atv_r_250_plastics`,
  `atv_r_250_exhaust`, `atv_r_250_engine`, `atv_frame`, `atv_r_250decals`), only `atv_r_250_exhaust.psg`
  has `250_Exhaust_Particle01`. The 450F build's `atv_r_450f_exhaust.psg` has the same node name. The
  exhaust part's root is the identity, and the node sits at (0.64, 1.96, −3.21), back right. Every part's
  `Placement` is `bike`, which is not a node of `250r_atv_chasis.psg`.
- **`main.rs`.** The quad's particle loader (`load_atv_fx`) now looks through the build's part models
  (`atv_part_hang`) for a node the chassis lacks. It takes the first part that has it. Each emitter's hang
  is now an `AtvHang`: a chassis node, or a part's node with the chassis node the part is placed on. For a
  placement the chassis lacks (`bike`), that is the quad's scene node, as `quad_draw::part_at` draws the
  part. Each frame, the part model's nodes are composed under that matrix. The shell says
  "Right Exhuast rides 250_Exhaust_Particle01, on the build's part atv_r_250_exhaust".
- **The slot's pack was stale.** s1's `mods/machines/untamed_atv` predated MD206 (no particle list,
  cameras or sounds in `mod.json`). I swapped in s2's copy, which is MD206's.
- Nothing in `../opentennis` covers other games' packs, so I took nothing from it.

## What the original does

Not read for this task. The particle list names the node, and the part model is the only place the
pack has it. That the original searches its sub-objects' nodes too was taken from the node being there,
not from SLUS-21701's code.

## How it was proved

- **Shell** (`crates/openmx/tests/untamed_atv.rs`
  `the_untamed_quad_says_its_cameras_engine_and_particles_are_the_packs`): the shell says the exhaust
  rides the exhaust part, no emitter is left off, and 8 ticks after the switch (under the emitter's
  15 ft/s) "Right Exhuast" particles are in the air. All 4 `untamed_atv` tests pass, and so do the 23
  `atv_camera_sound` tests (Unleashed's quad).
- By hand: at 34 and 38 frames there are 3 and 8 exhaust puffs. At 45 frames (24.7 ft/s) 2 are left, and
  at 60 none, as `f = throttle × ½ × (MaxSpeed − speed) / MaxSpeed` says.

## Not verified / not 1:1

- **Not picked out by eye.** One shot at tick 36 (`context/shots/md252/view.png`, software renderer)
  shows a grey trail behind the quad, but the few puffs can't be told from the tire dust at that size. No
  PCSX2 comparison. The smoke rides the same matrix the part is drawn at, so it should leave the drawn pipe.
- **How SLUS-21701 finds the node was not read.** The port takes the first part model with the name, and
  composes all of that part's nodes, its root's local included. The bike's exhaust rig
  (`particles::sim_rig`) leaves the part root's local out instead. Here the root is the identity, so the
  two agree for this pack; which one Untamed does was not checked.
- **Placement `bike`** (not a chassis node) hangs the part at the quad's scene node, as the shell draws
  the part. Not checked against SLUS-21701.
- **The 450F build was not ridden.** Its exhaust part has the same node name, but the pack's `mod.json`
  lists only the KTM 250 build.
- **Other seats' quads** online use this machine's hangs, as they used its chassis nodes before.

## Next

Done.
