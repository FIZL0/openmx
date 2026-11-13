# 1 — race second file (FINAL)

Task S299. 2026-11-13.

## What I did

- Found where the race activity's two race-file names come from and ported it as
  `race::race_file_names(track, tdf, ai_tdf)` (crates/game-sim/src/race.rs). Its doc says what each
  file holds. `TrackFile`'s doc points there.
- Two tests in crates/game-sim/tests/race.rs (proofs 39–40 in its doc):
  - `sx01s_ai_race_file_is_the_activitys_second_track_object_in_the_image`: S196's sx01 image.
  - `the_race_activitys_second_file_on_every_track`: a scan of every `_2.res` on the disc.
- oracle.txt: a note on the second load, and the name `activity_race_setup`.
- S282c had already found that the second file is the AI's net (`<track>AI.tdf`, ai+0xdc) and proved
  its paths' x/z, lengths and branches against the s282c race image
  (tests/ai/path_nets.rs). This task covers the rest: the name, the words that test skips, the
  heights, the gates, and every track on the disc.
- Nothing taken from opentennis (it has no race files).

## What the original does

- The base activity's setup (`./kit fn 0x2f8160`) reads two strings from the activity's settings,
  `TDF` and `AITDF`. If one is missing or empty (or there are no settings at all), it uses `"%s%s"` of the
  world's track name (world+0x140) and `.tdf` / `AI.tdf`. Then it writes act+0x184 and act+0x288 =
  `"%s%s\%s"` of `Tracks\`, the track name and the file. So sx01 gets `Tracks\sx01\sx01.tdf` and
  `Tracks\sx01\sx01AI.tdf`.
- The load (`./kit fn 0x2f8610`) runs the same steps for act+0x288 that it ran for act+0x184, but only
  once the first file has loaded: the archive lookup, track_ctor, track_file_load into act+0xbc, the
  ground snap with a1 = act+0xbc (`./kit dis 0x2f8610`, `lw $5, 188($16)` before the second `jal`),
  node_add_child onto act+0x50 with mask −1 (the first file uses 0x408), +0x38/+0x3c = 125/75, and
  ground_query on its line's and grid gate's centres. If the second file is missing or does not read,
  the load returns 0.

What the second file holds, from the disc scan:
- All 46 race tracks (nat01–22, sx01–24) have `<track>ai.tdf` next to `<track>.tdf` in
  `Tracks\<track>_2.res`. The ten freestyle archives have neither; their one race file is
  `<track>mr1.tdf`. No `.aty` on the disc mentions a tdf, so the names are always the defaults.
- Every AI file reads with the same reader. Each holds more paths than the race file (5–55 against 1–2)
  with more links than paths, so it is a branching web, and every path leads on. It has no
  checkpoints, has a grid gate and a line (a third gate only on nat01's), and uses the race file's own
  scale and offsets. Its path flags add only 0x08 and 0x20 to the reader's 0xc1.
- Words the race files leave at their defaults are used in the AI files: the paths' tag-0x18 word
  (`word_0c`) and the nodes' tag-0x12 word (`word_44`) hold float bit patterns, and `word_4c` is 0
  or 1.0.

## How it was proved

- **Save-state capture** (S196's `s196-sx01.ee.bin`):
  - The world's track name is `sx01`. act+0x184 and act+0x288 are `race_file_names("sx01", None, None)`.
  - sx01AI.tdf is looked up by that name (ignoring case) in `Tracks\sx01_2.res` and read. It is
    walked, then snapped on sx01's terrain off the disc (`world::Ground`). The result matches act+0xbc
    word for word: the header, tunables and counts (38 paths, 444 nodes, no checkpoints); every path's
    flags after the walk, length, words and links; every node's words, span and all four lanes of its
    three points; and the grid gate and the line, all words, centres on the ground included.
- **Disc scan** of all 56 `_2.res` archives in MX.PSR, as listed above.

## Not verified / not 1:1

- **`race_file_names` with a key set is proved only by reading the code.** No activity file sets
  `TDF` or `AITDF`, so the image only shows the defaults. The settings reader itself
  (`./kit fn 0x14b5e0`) is not ported or oracled.
- **The AI file's own words are not given meanings.** The paths' `word_0c`, the nodes'
  `word_44`/`word_4c` and the path flags 0x08/0x20 are read and checked bit for bit, but what reads
  them is the AI follower (S282b), which is not ported.
- **node_add_child's mask (−1 against 0x408) is not ported.** game-sim has no scene node tree for
  track objects.
- **What happens when the second load fails is not followed.** The load returns 0, and I did not read
  its caller. No disc track lacks the file.
- **Only sx01 is checked against an image.** The other 45 AI files are only proved to read, by the
  scan.

## Next

None.

(Gate note: my first `kit done` went red in three openmx web.rs browser tests, starting with "no CD001 at sector 16".
I never touched those tests. Alone they passed 13/13, and main had no fix for them, so I counted it as a load flake
and reran.)
