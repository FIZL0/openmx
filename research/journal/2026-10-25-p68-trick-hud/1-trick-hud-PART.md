# 1 — trick hud (PART)

Task P68. 2026-10-25.

## What I did

P121 had already given the port's own HUD (`--overlay hud`, the default) a trick pop-up, a combo total
and a camera toast off S79's score card. The original's HUD (`--overlay original`, the disc's art and
fonts) had none of them, and its score read 0. This task adds them there:

- `overlay.rs`: the Stunt movie (the trick's name over its points, in the disc's
  `yearlingdashboldoblique_18s` face, orange 255,153,0) and the info text (the camera preset's name in
  `yearlingdashbold_20`, white), at their layouts' places in a whole screen and in each split
  (`Place::stunt`, `Place::info`). The text is the original setter's: `stunt_name` (`BackFlip + %s`),
  `stunt_points` (`%d`, or `%d  X %d` of points ÷ n and n), `combo_total` (`%d Stunt Combo Total`).
  `Art::draw_pops` / `Art::sprites_pops` draw them on the CPU and the GPU paths.
- `ride_hud.rs`: `Tracker::pops` turns the tracker's state into those lines, with the same pop-in, the
  1 s hold of a landed chain's total and the 0.25 s play-out as P121's pop-up. The camera toast now
  stands for the original's 3.0 s (it was 1.5 s) in both HUDs.
- `main.rs`: the original HUD's score is the rider's banked total. Each view's tracker follows its own
  seat's card (`track_score`), so a split or an online ride shows every player their own tricks, points
  and camera.
- `tests/trick_hud.rs` (new): the formats; recording2's captured score card replayed through the
  tracker; a camera press; and with the disc the drawn lines' size and place.

## What the original does

- Camera button: `./kit fn 0x00200330` calls `./kit fn 0x002421c0` with 3.0 s, the UI manager, the new
  preset's name (preset +0x30, or "Helmet Cam") and the viewport. That queues the info text (up to 3
  messages, `./kit fn 0x002a49b0`). No sound is called on this path.
- Trick text, `./kit fn 0x002a5660`. With one node it writes `%s` (or `BackFlip + %s` while the
  listener's +0xa0 flag is set) over `%d`. From two nodes the two Stunt movies alternate by the count's
  parity: the older node plays out of one while the newest shows in the other as `%d  X %d`, its
  (int)points ÷ n and n (`./kit dis 0x2a5a80 0x2a5b10`). A landed chain of two or more shows
  `%d Stunt Combo Total` over the banked total and holds it while its clock is under 1.0 s
  (`./kit fn 0x002a6940`, `./kit fn 0x002a5ba0`). None of the listener's functions calls a sound.
- Layouts (fstyleui.res): `infotextoverlay1.lyt` 320,399 (horizontal 320,50, vertical 160,224);
  `stuntscoreoverlays.lyt` Stunt 320,373 (horizontal 320,182, vertical 160,360). The addresses are in
  research/oracle.txt under P68.
- Captured (S79's `recording2-score.csv`): `BackFlip + Taunt` stays up at flip stage 3, frames
  2581–2591, which P140's frames of the film show. So the port shows `BackFlip +` at any nonzero flip
  stage while the newest node is not the backflip itself.

## How it was proved

- A save-state capture: `context/capture/s79/recording2-score.csv`, the original's score card a tick at a
  time over the human's recording2. The test replays it through `Tracker` and checks it against P140's
  frames of the film of the same ride: `BackFlip + Taunt` / 3240, 3990; `BackFlip` / `10000  X 2`;
  `2 Stunt Combo Total` / `24160` for every tick of the second after the bank and gone 1.25 s later;
  `CanCan` / `3740`; the Taunt bailed at 3330 playing out with the total still 55190.
- Looked beside a screenshot: P121's PCSX2 frame `pcsx2-combo.png` (s5 `context/shots/P121/`). The
  original's name inks x 242–399 and lines 355–366 (layout px, the frame's 480 lines ÷ 480/448); its
  `3160` inks 282–361 and lines 375–396. The test holds ours to within 2–4 px of these; we draw
  242–400, 355–365 and 284–362, 375–394.
- `./kit check -- -p openmx --test trick_hud --test ride_hud --test overlay --test hud`: 98 tests
  passed. The full gate in `./kit done` was red only on replay.rs
  `the_slip_cases_replay_from_the_save_state` (S58 'turn' parts at 75, not after 82). It is just as red on bare
  HEAD ecd23368 in a scratch worktree, and it is S119's (main red since 1cdaee5a). The one other red, the lobby
  LAN test, passed on its automatic rerun (a load flake).

## Not verified / not 1:1

- The camera info text is not checked against PCSX2. I took no frame of a camera press. Its place
  comes from the layout file; its size is the font's own (1.0); its fade is my choice
  (`TOAST_FADE`, from P121). The 3.0 s is from the call's argument.
- The Stunt lines' sizes come from one frame. The original's face is drawn flatter than the font
  (0.91 across × 0.70 down for the name, 1.82 × 1.75 for the points). That is measured, not read from
  the movie's own transform.
- The original alternates two Stunt movies: the older node's line (e.g. `Taunt` / `2080`) plays out
  while the newest pops in. The port shows only the newest line. The older line's play-out is not
  drawn.
- `BackFlip +` at flip stages 1 and 2: the +0xa0 flag's writer was not pinned. Stage 3 is shown by
  the capture plus the film. Stages 1–2 are my inference.
- A single trick's land: the 0.25 s play-out is P121's measure from the film, not re-measured here.
- The split places use each layout's numbers in its own view's pixels, as V28 did for the
  speedometer. I filmed no split.
- Sounds: none found on the camera path or in the stunt listener's functions. A sound from a movie's
  own action script would not show there and was not looked for.

## Next

The work is done and committed on task/P68, and pushed by hand, because `./kit done` cannot pass while main
is red (S119). Once S119 lands, run `./kit done P68 "<message>"` from this branch, rename this file -FINAL,
and the box ticks. Nothing else is open.
