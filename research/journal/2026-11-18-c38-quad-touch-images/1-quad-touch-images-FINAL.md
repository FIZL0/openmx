# 1 — quad touch images (FINAL)

Task C38. 2026-11-18.

## What I did

- Looked for an ATV race state first. MD166's `context/mxvatv-ps2/md166/atv-race.p2s` (Desert Oasis, Open
  Class race, ATV) holds five quads, and its RAM already had two AI quads' records naming each other.
  That image is mid-touch, so it has no "frame before".
- research/c38/ (git-tracked):
  - `stage.py`: copies that state into the copy's slot 7.
  - `watch.sh` + `poll.py`: load slot 7 and log the five records every frame (count, kind and `+0x84` of
    each entry); with a frame range, save a state every frame in it (slots 10..).
  - `images.py`: read saved states back (their own vsync, each quad's body and record entries) and write
    `.ee.bin` / `.spr.bin`.
- Scratch kit.toml: `context/c32-atv-kit.toml` (SLUS-21104, vsync `0x3fa6d8`), KIT_PCSX2=6.
- Pass 1 (speed 1.0, 450 frames, no input): the pair parts by 37477 and touches again at 37523, every
  record empty at 37521. Pass 2 (NominalScalar 0.05, so a save fits in a frame): the same onset, states
  saved for 37513..37529. Kept 37521 and 37523 as `context/mxvatv-ps2/c38/c38-<vsync>.{p2s,ee.bin,spr.bin}`.
- Put the copy's ini back to NominalScalar 1.0, removed my slots, stopped the copy.
- Nothing taken from ../opentennis (no vehicle contacts there).

### Why AI quads and not the player's

The brief asked for the player's quad steered into a neighbour at the start. I took the AI pair instead:
- `/dev/uinput` is root-only, so steering needs a hand-timed tape through the race's menus and start.
- The AI pair gives a clean onset with no input at all, which replays the same way (pass 2 matched pass 1).
- The record pass and the kind 1 hand-over are the same code for every quad's record (all five share
  vtable `0x003b9290`), so it is the same proof target for MD98a.

## What the original does

- Record = `*(*(quad + 0x1434) + 0x90)`; body = `quad + 0xb8`. Full addresses in oracle.txt `# C38`.
- At 37523 each of the pair's records holds 4 entries, `+0x40` (touched) = 1.
  - Every `+0x84` is the other record; `+0x88` (mover) is 0; `+0x80` is `0xffffffff`.
  - 0x01a511b0 kinds 0, 1, 1, 1; 0x01aa5020 kinds 1, 0, 0, 0.
  - Each kind 1 entry is the other record's kind 0 entry at the same index: same point, normal turned round.
- The touch lasts to 37531 (counts 4, 5, 5, 5, 4 in the saved states 37523..37529, 4 in pass 1 at 37531).

## How it was proved

A save-state capture: the two images' own vsync words read 37521 and 37523, and their record lists were
read straight out of the RAM (`images.py`). The onset frame matched across two runs from the same state.

## Not verified / not 1:1

- The touching quads are two AI quads, not the player's quad at the race start the brief named: reasons
  above. The player's quad is in the images (standing, ~450 ft back), touching nothing.
- That a whole Ee frame from c38-37521 gives c38-37523 was not run. MD98a will run it; save states at a
  frame boundary have done so for MD105's images, but this pair is unchecked.
- One poll read (37527 in pass 2) showed count 1 where the saved state says 5. It was a mid-frame PINE read
  at the slow speed. Only the saved states are used.
- The 0x90-byte entry's fields other than kind, `+0x30`, `+0x40`, `+0x80`, `+0x84`, `+0x88` were not read.

## Next

MD98a, on these two images.
