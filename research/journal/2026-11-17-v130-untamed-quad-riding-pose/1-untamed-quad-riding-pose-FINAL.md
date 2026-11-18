# 1 — untamed quad riding pose (FINAL)

Task V130. 2026-11-17.

## What I did

- **No change to the compare.** I read how game-render's quad clip driver (`quad::Driver`) feeds the
  animate port:
  - Each `StartStunt` id from the step goes through `Driver::row_of`. On an Untamed quad that maps ids from
    49 up to 8 lower (MD144b), which puts them in the 46-row table's numbering.
  - The driver picks the riding pose itself (`RIDING_POSE`, 46) when the quad is in the air on no stunt.
  - So `Anim::next_stunt` is always in table rows. When animate compares it with 46, that matches Untamed's
    animate comparing its own id with 54.
  - Shifting the compare to 54, as the task text proposed, would have stopped the air pose from ever
    blending on an Untamed quad.
- **Wrote the reason down** in `row_of`'s doc comment (`crates/game-render/src/quad.rs`).
- **Two tests.**
  - `game-render` `quad::tests::an_untamed_quad_in_the_air_blends_the_riding_pose`: with Untamed's shift,
    a quad in the air on no stunt is on 46 and plays the same blend (`clips [48, 47]`) as Unleashed's.
    A `StartStunt(54)` (Untamed's riding pose id) lands on row 46 and blends.
  - `openmx` `tests/atv_clips.rs` `an_untamed_quad_in_the_air_blends_the_riding_pose`: the `untamed_atv`
    pack's own `atv_Race250P.phy.bpb`, ridden on Untamed's step (`on_untamed_step`), goes off MD138a's
    test ramp at full gas. Every frame in the air (74 here) is on the riding pose and plays two of
    `POSE_CLIPS`. The driver's shift is Untamed's on every frame.
- Took nothing from `../opentennis`: it has no quads or stunt tables.

## What the original does

- MD144a: SLUS-21701's quad animate is SLUS-21104's with its stunt bounds 8 up (46 → 54). Its clip fields
  moved too. Untamed's quad step hands out stunt ids 8 up from 41 on.
- MD144b: Untamed's ATV stunt table has the same 46 rows as Unleashed's. So "Untamed id − 8 against 46"
  makes the same choice as "Untamed id against 54".

## How it was proved

- The two unit/ride tests above. Both pass. I ran the ride test's binary with `--nocapture` to confirm it did
  not skip: "in the air 74 frames, all on the riding pose".
- By eye: `context/shots/md144b/untamed-air.png` (MD147's `air.p2s` screenshot) shows Untamed's rider in
  the air sat upright, hands on the bars. That is the riding pose, not a trick, which fits the original
  being on its riding pose id there.

## Not verified / not 1:1

- **No port shot of the Untamed quad in the air beside Untamed's.** This checkout's
  `mods/machines/untamed_atv` is the older install: `Aud`, `Part` and `Veh` only, with no rider set. So the
  shell would not draw Untamed's rider. Also, free09 has no jump near the spawn that a hand-written tape is
  sure to hit. The proof is the driver's choice of clips (tests above), not a picture.
- **The −8 mapping itself is still not checked against SLUS-21701's animate** by an oracle or a capture, as
  MD144b noted. This task's argument rests on MD144a's reading of the bounds and MD144b's row order.
- **The ride test uses `mxvatv_atv`'s stunt table**, not Untamed's own `ATV_Rider_Stunts.ini.bpb`. The
  installed Untamed pack lacks it, and MD144b found the two have the same 46 rows.
- **No Untamed stunt was ridden in the test.** Wheelie and stoppie ids are MD207's.

## Next

None: the task is done.
