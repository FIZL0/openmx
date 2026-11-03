# MD83a — the rider spheres' radii from the disc

2026-11-03. Brief: MD83 took the riders' nine sphere radii (0.4 0.4 0.75 0.75 0.75 0.5 0.5 0.75 0.75)
from MD37a's image. Find the file SLUS-21104 reads them from and read them from it.

## The file

`MXVSATV.PSR` was not extracted anywhere. I pulled it off `openmx-mods/iso/MX vs. ATV Unleashed (USA).iso`
with `bsdtar` into the session scratchpad. Then a scratch crate (`game_sim::archive::{Psr, Res}`) listed
every member, and every `.res` member's members, whose name holds `rider` or `.phy`.

The rider file is `Rider\Anim\mrider.phy`, the name MX Unleashed uses. It is both loose in the PSR and
inside `Rider\RiderPool.res`, byte for byte the same. SLUS-21104's data holds the strings `Rider\Anim\`
and `mrider.phy`. `mrider50.phy` differs only in the five `HitBottom*` keys, so its radii are the same.

Its 26 constraints include nine of `Type = Collision`, naming parts 15, 18 (the feet), 1, 3, 4, 7, 11, 14
and 17. Those parts' `Radius` keys are 0.4, 0.4, 0.75, 0.75, 0.75, 0.5, 0.5, 0.75, 0.75. That is the
moving list's order for each rider, so the list is MX Unleashed's rider spheres
(`ragdoll_contact::Sphere::read`) read off MX vs. ATV's file.

## The port

`atv_world::rider_radii(&Ini)` returns the nine radii through `Sphere::read`. `tests/atv_gather.rs`
reads `mrider.phy` (copied into `context/mxvatv-ps2/md37a/`) and gives each rider sphere
`radii[(slot - 18) / 2 % 9]` instead of the image's word. `same_fields` still compares every slot's
reach bit for bit against the image, so the 0.39999998 bits are now `Ini`'s parse of "0.4", the same
as the original's.

Nothing was taken from opentennis: it has no MX vs. ATV rider file.

## Checked

- `./kit check -- -p game-sim --test atv_gather`: TESTS PASSED, 4 tests.
- Mutation: shifting the radii by one part (`+ 1`) goes red in `mc02s_moving_list_is_the_originals` at
  slot 20, `reach`. That also shows the test ran and did not skip.

## Not verified / not 1:1

- **Which of the two copies is read.** The loose `Rider\Anim\mrider.phy` and `RiderPool.res`'s are
  identical, so it makes no difference here. The loader that opens it was not traced.
- **Which rider uses `mrider50.phy`.** It is not read. Its radii are the same, so all six riders' spheres
  match either way.
- **The rest of the rider file** in MX vs. ATV (its parts, pins, forces): only the collision spheres'
  radii are read for this task.
