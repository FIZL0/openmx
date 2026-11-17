# 1 — re extract mrider phy (FINAL)

Task MD117. 2026-11-17.

## What I did

- Pulled `MXVSATV.PSR` off the MX vs. ATV iso with `bsdtar` (recipe: memory mxvsatv-psr-off-the-iso) into
  the scratchpad. A scratch crate on `game_sim::archive::{Psr, Res}` wrote out every PSR member and every
  `.res` inner member whose name holds `mrider.phy` or `250`.
- `Rider\Anim\mrider.phy` (16,924 bytes) is both a loose PSR member and inside `Rider\RiderPool.res`; the
  two copies are byte-identical. Copied to the shared `context/mxvatv-ps2/md37a/mrider.phy`.
- `Veh\atv_*250*.phy` (eight files) were already in the shared `context/mxvatv-ps2/md37a/phy/` (since the
  11-13 merge). Each is byte-identical to its loose PSR member and to its copy in `Veh\vehicletype_1.res`.
- Kept both in `~/repos/openmx-slots/kept-context/md37a/mxvatv-ps2/md37a/` (`mrider.phy`, `phy/`, file
  names lower-cased as the tests read them) and listed them in that folder's README.txt.
- With the data there, atv_gather's `the_quads_nearby_list_is_the_originals` ran and failed only its
  palm-row count: its six palm rows named frames of MD37a's old race-jump capture (35805, 35851–35863),
  and C28 re-made that capture on 11-05 with new frame numbers (21459–22057). A one-off print in the test
  showed both palms in every C28 race-jump row from 21535 to 21741. The test now names 21535, 21537,
  21599, 21661, 21739, 21741.
- atv_quad's `the_quad_images_wheels_are_their_phy_files` ran only on `atv-r-spawn`: MD37a's other five
  images are gone. Added C28's four images (`c28-atv-{r,s}-{spawn,air}.ee.bin`, quad at 0x150a990) to
  its list.

## What the original does

Nothing new: data extraction and test upkeep only.

## How it was proved

- `cmp` of every extracted file against its other copy (loose PSR member vs. `.res` member, and against
  the files already in context).
- `./kit check -- -p game-sim --test atv_gather --test atv_quad` passed 127 (6 + 121). atv_gather's
  nearby-list test compared 13,180 lists (6,300 from capture rows, 28,980 candidates) bit for bit against
  SLUS-21104's own gather, with both palms in all 6 palm rows.
- The tire test: each of `atv-r-spawn` and the four C28 images matches exactly one `.phy`
  (`atv_race250p` for the R quad, `atv_s_race250p` for the S quad).

## Not verified / not 1:1

- MD37a's old images `atv-r-air`, `atv-r-spawn-late`, `atv-s-spawn`, `atv-s-air`, `atv-s-spawn-late`
  (`.ee.bin`) are in no slot or kept-context, so the tests that list them still skip those rows: the tire
  test (now covered by C28's four images instead, but no `-late` image), atv_gather's vehicle-sphere test
  (`atv-r-spawn-late`, `atv-r-air`) and atv_quad's MD86 ride (`atv-r-air`). They were RAM images of states
  that C28 replaced with new ones on a new heap; they cannot be re-extracted, only re-captured.
- The new palm rows were chosen from what the port's own lists hold, but those lists are asserted equal to
  the original's on the same rows, so the choice rests on the original's gather.

## Next

None.
