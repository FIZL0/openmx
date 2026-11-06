# 1 — mxvatv pilot seat (FINAL)

Task P316. 2026-11-05.

## What I did

- `game_render::biplane`:
  - `mxvatv_seat(pelvis_box)`: where SLUS-21104 puts a flier pilot's root in model space. That is
    the model's `PelvisBoxPosition` itself.
  - `Biplane::seat` (the root's place, `PILOT_SEAT` by default) and `Biplane::with_seat`, which
    moves the four limb anchors to "node less the new seat". `Biplane::pilot` reads `seat`.
  - The module doc says why MX vs. ATV differs.
- `main.rs` (and so `android_so.rs`, its symlink):
  - `plane_drawn`, on the fliers pack: every livery's `Biplane` gets `with_seat(mxvatv_seat(...))` of
    the pack's `biplane.phy` read for the skin's model. Both liveries of a pick are the same model
    (`mxvatv_skin` keeps the pair).
  - `heli_pilots`: an MX vs. ATV helicopter (`KiowaRide::mxvatv` set) seats at
    `mxvatv_seat(phy.pelvis_box_position)`, which `Phy::read_mxvatv` already read for the model.
    Otherwise it stays at `kiowa::PILOT_SEAT`.
- MX Unleashed's plane and helicopter are unchanged: (0, 3.5, −3) and (1.25, 3.5, 3).
- New `crates/openmx/tests/mxvatv_pilot_seat.rs`, with two tests.
  - The RAM-image proof below.
  - The shell under `--trace-picture` draws skin 1's pilot root at (0, 3.2, −3.0) and skin 3's at
    (0, 4.4, 0.7).

I took nothing from opentennis, which has no vehicles.

**Departure from the brief:** the brief says to keep model one where it is now, at (0, 3.5, −3). The
original puts MX vs. ATV's model one 0.3 ft lower, at (0, 3.2, −3.0): see below. The brief also says
to seat the plane "as SLUS-21104 does for the skin's model", so I followed the original. Under
`--fliers mxvatv`, skins 1 and 2 now sit 0.3 ft lower than before. MX Unleashed's own plane does not
move.

## What the original does

- `./kit dis 4c1340 --len 0x170 -p atv` is atv_biplane_pilot_request, the twin of MX Unleashed's
  biplane_pilot_request. Its request block is at plane +0x730. It writes:
  - +0x8e0 (the base) = plane +0xc40. That is the model matrix's position: the same quadword as row 3
    of the matrix at +0x370, which the request also copies to +0x750..+0x790.
  - MX Unleashed's request takes the rigid body's centre of mass (body +0x50) as the base instead.
  - +0x8f0 = the body's quat (+0x190); +0x7d0 = velocity; +0x7e0 = rates.
  - +0x830 / +0x834 = &PelvisBoxPosition (+0x1330) / &PelvisBoxSize (+0x1340), which the reader filled
    by model.
  - The four Dummy node positions go to +0x7f0..+0x820, and the held flag +0x73c = +0x31c.
  - Phase and mix are MX Unleashed's 0.5·(1 − x) and 0.5·(1 + y) of +0x10f8 / +0x10f4.
- The body puts its root at base + rotation·offset. With the model origin as the base, the root in
  model space is `PelvisBoxPosition` exactly. MX Unleashed's sum `CofG + PelvisBoxPosition` does not
  apply: MX vs. ATV's `CofG` is (0, 3.5, −2.5), and the sum would put model one at (0, 6.7, −5.5).
- On every image, body position − model position has length |CofG|, so the model origin is the
  centre of mass less the turned `CofG`, as in MX Unleashed.
- The helicopter does the same. On both Kiowa images, a node sits at its model's pelvis box with the
  model's rotation (matrix at heli +0x370). I did not disassemble its request; the RAM shows the
  result.

| image | model | PelvisBoxPosition | root node in model space |
|---|---|---|---|
| biplane | one | (0, 3.2, −3.0) | (0.0001, 3.2000, −3.0002), rotation equal to 1.2e-7 |
| biplane3 (Spitfire) | two | (0, 4.4, 0.7) | (0.0002, 4.4000, 0.6999), 2.4e-7 |
| kiowa | one | (1, 3, 5) | (1.0002, 3.0000, 4.9998), 1.8e-7 |
| kiowa3 (Little Bird) | two | (−1, 5.1, 1.5) | (−0.9997, 5.1000, 1.4999), 1.8e-7 |

So the port's MX Unleashed helicopter seat (1.25, 3.5, 3) was ~2 ft back and ~0.5 ft high in MX
vs. ATV's model one. On the Little Bird it was 2.25 ft to the right and 1.6 ft low.

## How it was proved

- **Save-state RAM images:** `tests/mxvatv_pilot_seat.rs` reads MD80a's four images
  (`context/mxvatv-ps2/md80a/{biplane,biplane3,kiowa,kiowa3}.ee.bin`; the `.p2s` are in
  kept-context/md80a; `research/md80a/ram.py NAME` makes the `.ee.bin`). For each image it checks:
  - the pelvis box in RAM equals the port's `read_mxvatv` of the pack's file for the object's model;
  - on the plane: the request's base equals the model position, and its offset pointer is the pelvis
    box;
  - a matrix in RAM has the model's rotation (1e-5) at `mxvatv_seat` (1e-3 ft).

  I probed it once with a temporary panic: all four images ran and none was skipped. Each had 4
  matching matrices.
- **The shell:** the same file's second test runs `--fliers mxvatv --flier-skin 1/3 --trace-picture`.
  The roots are (0, 3.2, −3.0) and (0, 4.4, 0.7).
- **Shots:**
  - In PCSX2 (copy 1, a scratch SLUS-21104 kit.toml), I loaded biplane3.p2s and biplane.p2s and took
    snaps over AT-SPI (System → Screenshot), from the original's chase camera behind the plane. The
    Spitfire's pilot is a dark helmet in the canopy, just above the fuselage, inside the prop disc.
  - openmx's rear shot of skin 3 (`--shot-pose 2304,149,2312,180,-14,22`) shows the helmet in the
    same place. The side shots of skins 1 and 3 (`--shot-pose 2335,146,2281,270,-12,45`) show:
    - skin 3: the pilot's head in the Spitfire's cockpit, with nothing through the bottom;
    - skin 1: the pilot behind the top wing as before.
  - The shots are not committed (scratchpad only).

## Not verified / not 1:1

- **The original's Spitfire seen from one angle only.** The PCSX2 shot is the state's chase camera
  from behind, at the game's 347×260 snap size. There is no pad here (uinput is root-only), so I
  could not move the original's camera to a side view. The side comparison rests on the RAM root
  position plus openmx's own side shot.
- **The helicopter's pilot request was not disassembled.** That the Kiowa's base is also its model
  matrix is inferred from the root node sitting at the pelvis box on both images, not read from code.
  I took no PCSX2 or openmx shot of the MX vs. ATV Kiowa's pilot; its glass is probably as opaque as
  MX Unleashed's, which P216b1 found.
- **Limb holds.** The MX vs. ATV plane's limbs are still held to the model's four Dummy nodes, as MX
  Unleashed's are. The request does write those nodes (+0x7f0..+0x820), and held is 0 on both
  images, as on MX Unleashed's grounded states. The bones of the held pose were not compared with
  the images' palettes, as P216b1 did for the unheld Kiowa. The Kiowa stays unheld, as in P216b1;
  whether SLUS-21104's Kiowa holds its limbs was not checked.
- **Only grounded, level states:** each image is at the spawn. The root formula does not depend on
  the stick, but no banked or flying image was compared.
- **Mid-ride skin changes:** `plane_drawn` is built once at the ride's start. A model change mid-ride
  (P315's picker, not on main) would need the seat redone. P315 is where that lands.

## Next

None.
