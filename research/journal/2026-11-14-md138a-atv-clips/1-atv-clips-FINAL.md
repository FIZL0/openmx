# 1 — atv clips (FINAL)

Task MD138a. 2026-11-14.

## What I did

- `game_render::quad::Driver` keeps the quad's stunt word the way the original's bike does: it starts on
  `Call::StartStunt`, runs a clock, ends at the row's Duration (a backflip or 360 is held while the flip /
  spin is active), and falls back to 46 in the air or 255 on the ground (255 again once wrecked). On that
  word it runs MD65c's `game_sim::quad::animate` and keeps what that function plays (`Played`: one clip or two
  blended, their weight and fade, plus the clip that was playing before the fade).
- `quad_draw::stunts` builds the clip table from the pack's `ATV_Rider_Stunts.ini`. A row marked
  UsedForBike gets the pack's `ATV_Bike_<name>` machine clip. A row marked HasAnimation gets the rider's
  `ATV_rider_<name>` clip. The build `.mcf` files give the three riding-pose clips
  (`MachinePack::build_clips`).
- `quad_draw::chassis_locals` lays the chosen clip, faded over the clip it replaced, on the chassis
  skeleton, and `quad::pose_from` builds the quad's model pose from that. `quad::rider_palette_stunt` plays
  the rider's stunt clip from StartStunt up to EndStunt (held on its last frame).
- `vehicle::Quad` owns a `Driver` and ticks it after each step. `AtvRide.stunts` reaches it from the shell's
  look, through `main.rs`. The picture trace now prints the chassis's clip and the stunt.
- `quad::Ride::controls` now passes the trick buttons into the steps' byte, so R1 alone arms the backflip.
  Before this the byte was never written from controls.
- Tests: four in `game-render` (`quad::tests`), and `crates/openmx/tests/atv_clips.rs`.

## What the original does

- The quad's clip binder (`./kit fn 0x004dd280`) binds `ATV_bike_%s` only for stunt rows marked
  UsedForBike. In mxvatv_atv's table those are rows 0, 6, 7, 12, 15, 16, 22, 33, 35, 36, 44 and 45 (taunt,
  nacnac, flatliner, kod, the two indian airs, tsunami, 9 o'clock nac, ragdoll, Varial, both 360s). **Row 41,
  backflip, is UsedForBike=F.** The original binds no `ATV_Bike_BackFlip`, and during a backflip its
  driver (MD65c's port) plays clip 47, Air_Mid, at a weight of 0.5. That is why the brief's test name ("a
  quad backflip plays ATV_Bike_BackFlip") does not match the original. This test checks what the original
  does, and the name was changed to say so.
- The rider's binder (`./kit fn 0x0027ac30`) binds `ATV_rider_%s` for every row marked HasAnimation, and
  start/end stunt play and stop the rider's clip on it (`./kit fn 0x004e6370`, `./kit fn 0x004e4db0`, S223).
- Each build `.mcf` maps ATV_Bike_Air_Fwd, Air_Mid and Air_Bck all to `ATV_Bike_Air_Mid.MOT`, so the
  riding pose's blend is one clip three times.
- The stunt clock and the 46/255 fallback are in `./kit fn 0x004e5d20` and `./kit fn 0x004e49c0`.
  `./kit fn 0x004e1b90` clears the restart flag. Offsets are in research/oracle.txt, `# MD138a`.

## How it was proved

- From reading the code: the binders, start/end stunt, stunt idle/request (above), and the pack's ini and
  mcf files.
- `game-render` unit tests drive `Driver` with hand-made calls: on the ground (clip 47 at 0.5), riding in
  the air (blend [48,47]), a backflip (rider clip 41, chassis 47 at 0.5), and a stunt with a quad clip of its
  own (that clip, at the stunt clock over its length).
- `crates/openmx/tests/atv_clips.rs` rides the shell's quad (`vehicle::AtvRide`) off a ramp of the test's
  own. On the lip, R1 alone with the stick back arms the flip and the stick forward fires it. In the air,
  the steps emit StartStunt(41) (frame 108 of 400). From there to the flip's EndStunt (frame 159), the
  rider plays `ATV_rider_backflip`. EndStunt stops the rider's clip, and the stunt word stays 41 while the
  flip is still active. On every frame where the word is 41, the chassis plays clip 47 at 0.5. The pack
  table's backflip row has no quad clip, and other rows do.
- The test runs on the pack, and skips without `mods/machines/mxvatv_atv`.
- `./kit check --all`: one red, `mxvatv_fliers` skin names ("Plane 3" vs "Plane (3)"). The cause was this
  slot's gitignored, stale `mods/machines/mxvatv_fliers/mod.json` (it had no skin names). With s1's copied
  in, it passes. That file is outside git and is not part of this change.
- `game-sim --test atv_quad` (131) and `openmx --test atv_picture` still pass with the button change.

## Not verified / not 1:1

- **ATV_Bike_BackFlip is not played.** The brief asked for it, but the original binds none for the
  backflip (UsedForBike=F), so the port does not play it either.
- **The rider's motion-set row and lean choice is not ported.** The rider anim words that hold it
  (+0x310..+0x344) have no reader found yet, so MD138's own choice stays: air / stopped under 1 ft/s /
  riding, leaning with the stick. Filed as MD162.
- The stunt word is a stand-in for the bike's (+0x1528, its clock, restart +0x1650), kept outside the sim
  and ticked after the step. The original runs it inside the vehicle update, so its order against the step
  and its clock may be off by one dt.
- `animate` runs on a default `Body` and `NetClock`, with the entry +8 / mode 1 conditions taken as met.
  Nothing has been compared against the original's quad.
- How the rider's stunt clip plays is the port's own: it replaces the riding pose entirely and holds its
  last frame until EndStunt. The original's blend of the stunt layer is not read.
- No PCSX2 capture or screenshot compares the picture with the original's.
- The ramp in atv_clips.rs is the test's own world, not one of the game's.
- The test's quad lands that flip wrecked (R1 is held all the way round, so the torque never decays).
  That is the tape's doing, and nothing here checks how far the flip turns.
- The quad's 360s emit no StartStunt in the sim steps (outside this task), so their clips never start.
- Passing the trick buttons into `quad::Ride::controls` changes the sim: the backflip can now arm in the
  shell.

## Next

- MD162: find the reader of the rider's motion-set row and lean words and port it.
