# 1 — ride stick dead zone (FINAL)

Task I7. 2026-10-20.

## What I did

- Played the human's `recording2.p2m2` back on the original (copy 4) and recorded
  `watch/acceptance.toml` plus the player record's left stick: bike `+0x20` → `+0x324` (x) and
  `+0x328` (y). The output is `context/capture/i7/recording2-sticks.csv`: 1,894 rows, frames
  7232..11018, the same range C6 got. The script is `playback/play_capture.sh`, which is C6's recipe
  and C6's AT-SPI helpers with two stick fields added to the spec.
- New test `crates/game-sim/tests/ride_sticks.rs`, `the_bike_reads_the_players_dead_zone` (see below).
- **Moved the ride onto ±0.3.** `Controls::from_pad` (ride.rs) now reads the left stick through
  `Pad::axis_through(.., &AxisRange::PLAYER)` instead of `Pad::axis` (I2's ±0.2).
  - `Pad::axis` is unchanged. It is still the main menu's range.
- HUD (`openmx/src/hud.rs`): both sticks' crosses and dead-zone boxes now use the player's range on
  the bike as well. Before, only the plane's did. In a ride all four of the player's consumers
  carry ±0.3 (I6's RAM scan), so there is no longer a bike/plane split.
- Keyboard ramp (`openmx/src/pad.rs`): `KEY_TO_EDGE` stays at 0.18 s. Its doc now says the dead zone
  is crossed in ~54 ms, not ~35 ms. `a_tap_no_longer_slams_the_game_s_own_axis` now reads through
  `Controls::from_pad`. Its "third"/"fourth" frame is the same fourth step it always was (0.37 of the
  travel, past 0.3).
- `platforms.rs`: the ride's pinned vectors moved, because the canonical run steers inside the band.
  I spliced in seven constants from `print_the_vectors`: `HASHES`, `TRACE_DIGEST`, `POS_Y`, `VEL_Y`,
  `FINAL_DUMP_DIGEST` and `FIXTURE_TEXT_DIGEST` changed, and `FINAL_DUMP_HEAD` came out identical.
  `FINAL_DUMP_LEAVES`, `EVERY_LEAF_HASH` and `EVERY_LEAF_DUMP` did not change, so the state type did
  not change. `HUNDRED_TICKS` did not move.

## What the original does

The bike's player record holds, at `+0x324`/`+0x328`, the left stick through the player's own range
(centre 0, −1..1, dead zone ±0.3). This is the same range and the same arithmetic as the plane's
(I6; `./kit fn 0x25e910` is the per-tick copy). Report bytes 90..=165 read exactly zero.

## How it was proved

**Live capture, tick for tick:** `the_bike_reads_the_players_dead_zone`. All 1,892 ticks (rows
2..1893) are fed through the port's `Manager`. On every tick:
- the 16 held flags of the game's own control state (`ctl*.held`) must match the port's;
- the two player-record sticks must match `Controls::from_pad(pad).stick` bit for bit.

Result: **1,892 of 1,892 ticks bit-exact**, with no exceptions and no torn rows.

There are 174 axis values in the band where the two ranges disagree (bytes 90..102 and 153..165).
±0.2 gives the wrong answer on **every one** of them, and the test asserts that. (A first, misaligned
pass already scored ±0.2 at 0–3 of the band against ±0.3's 146–150.)

**The alignment.** Row *r*'s tick polled the buffer stamped `frame − 2`, which is the row before's
*newer* buffer. `PadTape::from_capture` uses this row's older buffer (`frame − 1`). I measured this
with `playback/which_buffer.py` (sticks) and `playback/which_buttons.py` (held count). Both read the
capture on stdin.
- On this capture, every row where the candidate buffers disagree includes f−2: 815 of 815 for the
  sticks, and every row for the held count.
- C6's `recording2-freeride.csv` gives the same held-count result (f−2).
- C7's `biplane.csv` gives f−1 for both.

So the one-vsync shift is the recorder's phase on recording2's playback, not the bike reading a
stale stick: the game's own control state shifts with it. The test checks the held flags first, so a
wrong alignment fails there before it can say anything about the sticks.

## Not verified / not 1:1

- **The right stick (+0x32c/+0x330).** The bike port reads no right stick, and I captured only the
  left one. ±0.3 for the right stick rests on I6's RAM scan alone; the HUD now draws it through that
  range.
- **Why recording2's capture phase is f−2 when C7's is f−1.** Measured, not explained. It probably
  explains S30's `the_humans_tricks` failing on C6's capture, since `PadTape::from_capture` assumes f−1, but I
  did not test that. Outside I7.
- **Cross-architecture confirmation of the new `platforms.rs` vectors.** They were generated on this
  x86-64 host only. The cross-arch runs need an upstream toolchain that this machine lacks.
- **The keyboard ramp's 0.18 s** is a feel choice with no original to match, as before. I re-derived
  its doc for ±0.3 but did not retune it.
- **Tick 0's input** (row 1) is not checked: its poll is the buffer in row 0, which is before the
  manager has any history. The test starts at row 2.

## Next

Nothing for I7.
