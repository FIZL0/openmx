# 12 — closing MD75 (FINAL)

Task MD75. 2026-11-04.

## What I did

- Reopened after MD75c, MD75d and MD86 landed. Since then MD75e and MD75e1 have landed too, and between them
  they make the brief's proof. Every MD75 subtask (a, b, b1, c, c1, c2, c3, d, e, e1) is ticked. This
  session added no code. It checked that the proof runs here and closed the parent task.

## What the original does

See `1-body-and-world-step-FINAL.md` through `11-whole-frame.md`, and `research/oracle.txt` under `# MD75`
and "MD75e".

## How it was proved

- The brief's proof is `tests/atv_quad/frame.rs` `the_spawned_quads_whole_frame_is_the_originals`.
  SLUS-21104's world step runs one 1/30 s frame (four substeps) off MD37a's `atv-r-spawn.ee.bin`. Only
  the ground query (atv_ground_query) is hooked, answered on both sides by `atv_ground::Terrain`.
  `quad::frame` runs over `image::read_ride`'s read of the same image. All 2,923 compared words of the quad,
  body, wheels, nodes and collidables are equal, and so is the whole rigid body.
- Rerun this session with `--nocapture`, so it ran and did not skip, off the image in this slot's
  `context/mxvatv-ps2/md37a/`. Its output: "63073 instructions, 16 ground queries; 2923 words equal, 386 of
  them changed". MD75e1's frame where the quad touches something gave "68534 instructions … 2923 words
  equal, 389 of them changed". Both tests passed, and so did `the_quads_physics_frame_is_the_originals`.
- Then `./kit check --all` (by `./kit done`): TESTS PASSED, 4,800 tests. `./kit audit` flagged only the 4
  leaks already on main (atv_wheel.rs:62/66, plan/DONE.md:519/533). They are not this task's files, so I
  left them alone and ran `done` with `--skip-audit`.

## Not verified / not 1:1

Carried over from `11-whole-frame.md` (still open, nothing new):

- The search's searched bit (`+0x126` bit 0x10) on the quad's own collidables is not carried. Nothing
  ported reads it.
- The 40 scene nodes past `atv_moto::NODES` are not carried. In the original their world-current bit goes
  down; in the port it does not.
- The world's other five preforce listeners (the second ATV and four more objects) run only in the original.
  In the proved frames they leave the quad's words alone. They are not ported.
- `PartMove` boxes are handed on by the port. MD75c1 and MD89 prove that move separately, not inside the
  whole frame.
- The whole frame is proved only off one image, at spawn, plus MD75e1's touching frame. Several frames in a
  row, a struck object or a gathered nearby list have not been run whole.
- MD37a's race-row csvs are in no slot, so the three oasis contact tests skip here.

## Next

Nothing for MD75. The gaps above are what a multi-frame ride proof would take up.
