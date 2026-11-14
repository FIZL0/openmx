# 1 — atv peak loads (FINAL)

Task MD133b. 2026-11-13.

## What I did

- Traced the shell's free09 gas run on `atv_Race250P` (mxvatv_atv.rs's tape, a temporary trace in
  `quad::Ride::wreck_on`, since removed). The quad takes off at tick 137 and lands at tick 207 at
  (−10, −37, 63) ft/s. That is 65 ft/s forward and 40 down, and it lands **onto the next jump's
  up-slope**: after the landing it climbs at about 18 ft/s. Its loads (side, vertical, fore-aft) are
  (9.74, 108.78, −6.84) on tick 208 and (13.25, 95.39, −20.53) on tick 209. It wrecks front (1) on
  209.
- **The limits are not 12.5.** MD133's journal quoted the `.phy`'s `FrontWreckSpeedLimit = 12.5`, but
  the loader multiplies every wreck speed by MPH (1.46666, `phy.rs`). The original's race quad holds
  18.33 front, 22.0 back, 14.67 side and 117.33/117.33/29.33 down (C28's race-jump capture, quad
  +0x13e8..+0x1410, every row). −20.53 is past 18.33. The side load 13.25 is inside 14.67, and the
  vertical 108.78 is inside 117.33.
- Read the original's own loads (+0x1250 side, +0x1254 vertical, +0x1258 fore-aft) out of C28's
  captures (the obj1 block). In race-jump, the original lands flat at 26 ft/s down: vertical 24.6,
  fore-aft −0.7. At row 66 it meets a face at 50 ft/s and **wrecks front with fore-aft −23.17**: the
  same clause, with a load the size of the shell's.
- No capture has the original landing at the shell's speeds, and an MX Unleashed world cannot be loaded
  into MX vs. ATV. So I ran the original's code at those speeds instead, in the interpreter. The new test
  `an_mxu_sized_landing_rides_out_and_its_face_wrecks_front_by_the_original_and_the_port`
  (crates/game-sim/tests/atv_quad/replay.rs) works like this:
  - It starts from race-jump off C28's `atv-r-air` image, with race-jump's pad.
  - On row 47 (a row before race-jump's own landing) it lays the body's velocity to the shell's: the
    horizontal × 1.55 and y −40, giving (63.7, −41.1, −19.4).
  - It runs 62 rows, each by the original's whole sim frame, with the port's pieces and tail beside
    it. Every RAM word must be equal each frame.
  - The original lands flat on row 49 at (63.6, −42.1). Its loads peak at vertical 69.12 with fore-aft
    −3.61, and it **rides the landing out**. On row 60 it meets the next face at 67 ft/s and **wrecks
    front, fore-aft −33.31** against 18.33.
  - The test holds these against the `.phy`'s limits in ft/s: the landing's vertical peak over 60 and
    within the limit, the landing's side and fore-aft within theirs, at least 8 rows riding before the
    face, and the face's wreck reason 1 with a fore-aft past the front limit at over 60 ft/s.
- So the original wrecks front whenever it meets a face at 50–67 ft/s, with fore-aft −23 to −33. The
  shell's quad meets a face at 65 ft/s with fore-aft −20.5. The wreck is the original's, not the
  port's loads, and the "wrecked 0 times" assertion would be wrong.
  `a_picked_quad_rides_and_leaves_the_bike_ride_alone` (crates/openmx/tests/mxvatv_atv.rs) now holds
  **exactly one** wreck (`, wrecked 1 times`) instead of none. A second wreck, or none, goes red.
- The slot had no `mods/machines/mxvatv_atv`, so the shell test was skipping here. I copied it from slot
  s1's (git-ignored) to run the shell.

## What the original does

The loads are the body's correction peak in the body's frame. The correction is the velocity the
contact solver adds per substep, plus the wheels' bottom-out taking the closing speed out along the
quad's up. The peak is kept per axis by magnitude over the frame's substeps and reset each frame. On a
flat landing the correction is mostly vertical. Landing on, or running into, a rising face takes speed
out along the quad's fore-aft axis, and that is what crosses the front limit. The clause order is
`handling::wreck::clauses`: fore-aft past back or front, side, then vertical.

The shell's slope landing has about 57 ft/s of closing speed along the slope's normal (40 down plus
65 forward against a rise of about 17°). The original's flat landing has 42. 69.1 × 57/42 ≈ 94, against
the shell's 95–109: the vertical peak scales as the original's does.

## How it was proved

- The original's sim frame in the interpreter off C28's `atv-r-air` image, with a laid velocity: 62
  frames, the port's frame in pieces equal on every RAM word every frame (`sim_chain` with
  `Pieces::Port` and the tail). The test is in atv_quad/replay.rs.
- C28's race-jump capture (PINE, every tick): the original's loads and wreck reason on its own face hit,
  and the wreck limits, read from the obj1 block (a scratch script, not committed).
- The shell's free09 run: `./kit check -- -p openmx --test mxvatv_atv`, with the pack in mods/machines/.

## Not verified / not 1:1

- **No capture of the original at the shell's landing.** The original's numbers at 65 ft/s / 40 down
  come from its own code in the interpreter, on a velocity laid onto C28's air image mid-flight. They
  are not from a PCSX2 recording of a real jump that big. The laid velocity is not one the game itself
  produced, but it is the same state the frame reads, and the twin shows the port's frame matches.
- **Not on free09's ground.** The shell's quad rides MX Unleashed's world through `quad::Ride` and the
  MXU collision. The proof's quad rides Desert Oasis through the port's Desert Oasis world step. The loads'
  arithmetic (contact solver, bottom-out, peak) is the same port code in both, proven bit-exact here and
  in MD37b's replays. The ground contacts that feed it on free09 are not compared against anything
  (MX vs. ATV cannot load free09). The shell's slope landing (vertical 108.8) is checked only against the
  scaling argument above.
- **The landing and the face are separate in the proof.** Desert Oasis' landing spot is flat with the
  face 11 rows on. Free09's landing is onto the face. That the original also wrecks when the two
  coincide is inferred from both parts, not run.
- **The shell test now pins one wreck.** Any later change to the quad's ground or the wreck test that
  moves the landing will move this count, and that will need a fresh look, not a quiet edit.

## Next

Nothing filed.
