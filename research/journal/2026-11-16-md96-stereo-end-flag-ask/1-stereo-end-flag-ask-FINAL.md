# 1 — stereo end flag ask (FINAL)

Task MD96. 2026-11-16.

## What I did

- **import-music's filler-pair `todo` line: already gone.** openmx-mods MD111 (946c449, 2026-11-16) made
  import-music follow B1: no filler `todo`, and a read that stops short of or past the end flag is the
  problem it reports now. Its `notes/music.md` was updated there too. Nothing left to do for this half.
- **Answered the MD7 ask** in openmx-mods' `notes/openmx-asks.md` (fc3d788, pushed to its main and to
  branch `openmx-MD96`): the entry now says the fix is done (B1 3f380b287, import-music since MD111), in
  past tense, and replaces "Not checked: whether MX Unleashed's own `.vig` songs have the same filler" with
  B1's answer: all 51 of MX Unleashed's stereo `.vig` (13 songs, 36 UI streams, `cannonball_stereo.vig`,
  `cheer.vig`) carry the same one `0x07` filler pair.
- openmx's own `notes/openmx-asks.md` (named on the task line) has no MD7 entry; it lists asks the other
  way, so it is unchanged.

## What the original does

Not read here; see B1's journal (`2026-11-04-b1-stereo-vig-end-flag`).

## How it was proved

Notes-only change. The MXU count comes from B1's disc probe, not re-run. openmx-mods' `check.sh` ran its
tracked-file checks (no game data, nothing over 1 MB), which passed; its `cargo test --workspace` step was
not run, since only a markdown file changed.

## Not verified / not 1:1

- **The 51-file MXU finding was not re-probed**: taken from B1's journal, whose probe was thrown away
  after; B1's test keeps only `Crowd\cheer.vig`.
- **The original's stream player was not read**, as in B1: that the game stops at the flag rests on the
  SPU's end-flag rule. The ask's entry says so.
- **openmx-mods' cargo tests were not run** for fc3d788: it changes one markdown file only.

## Next

Nothing.
