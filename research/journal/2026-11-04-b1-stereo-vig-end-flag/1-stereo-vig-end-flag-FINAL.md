# 1 — stereo vig end flag (FINAL)

Task B1. 2026-11-04.

## What I did

- **Checked MX Unleashed's own `.vig` files first** (a throwaway disc probe in `tests/audio.rs`, removed
  after): every one of the disc's 51 stereo `.vig` — the 13 songs in `Aud\Music`, the 36 UI streams in
  `Aud\UI`, `Aud\SFX\cannonball_stereo.vig` and `Aud\crowd\cheer.vig` — has the same shape as MX vs. ATV's
  pack songs: the end flag (flags `0x01`) on *both* blocks of the second-last pair, then one filler pair,
  flags `0x00`, header byte `0x07` (filter 0, shift 7), every data byte `0x77` — 28 frames of a flat 224.
  The header's size reaches exactly to the filler's end (`pairs = flagged + 2` in each). So the disc's
  songs played the same 28-frame tail, and the fix serves them and the packs alike.
- `audio::decode_stereo` (so `Clip::vig`) now stops after the pair whose block carries the end flag, on
  either channel, as `decode_mono` stops on a mono body. `Adpcm::read` (the song player) does the same:
  reading the flagged pair moves its cursor to the body's end, so the next read returns 0 and the player
  moves on as it did at the header's size. One helper, `ends_stereo`.
- Nothing taken from opentennis: its `hst-data/src/snd.rs` decodes mono VAG only.

## What the original does

The SPU ends a voice on a block whose flags have bit 0 set and bit 1 (repeat) clear, so the filler pair
after it is never heard on hardware. I did not read the original's stream player (how it feeds the SPU
from the `.vig` and where it stops); see below.

## How it was proved

- `tests/audio.rs` `a_stereo_vig_stops_at_its_end_flag`: a generated stereo `.vig` (three sound pairs,
  the third flagged, then two filler pairs) decodes to 3 × 28 frames through `decode_stereo`, `Clip::vig`
  and `Adpcm` read in 19-frame pieces; a flag on one channel's block only ends the pair too; the filler
  block decodes to all 224. Its disc half: `Crowd\cheer.vig` is 5294 × 28 frames (5295 pairs, flag on the
  5294th) and does not end on the flat 224.

## Not verified / not 1:1

- **The original's stream player was not read or captured.** That it stops at the flag rests on the SPU's
  end-flag rule and on decode_mono's existing behaviour, not on the game's `.vig` streaming code or an
  audio capture; music is presentation ("feels right"), and 28 frames of a constant offset is the whole
  difference, so I did not spend an emulator run on it.
- **Only `Crowd\cheer.vig` is checked against the disc in the test**; the other 50 disc files' layout
  was read once by the probe (above) and not kept as a test, since decoding the 13 full songs in a debug
  test costs seconds for no added coverage of the code path.
- **MX vs. ATV's pack songs were not re-run** here: openmx-mods' `import-music` builds `audio.rs` itself;
  the change adds no `crate::` use, but its tests and the pack's `todo` line are that repo's (MD95 filed).

## Next

Nothing for B1.

`./kit done` ran with `--skip-audit`: its 4 leaks are in `crates/game-sim/src/atv_wheel.rs` and
`plan/DONE.md`, which other tasks wrote and B1 does not touch (K23 skipped for the same ones; K26).
