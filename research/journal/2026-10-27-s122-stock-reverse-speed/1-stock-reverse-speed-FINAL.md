# 1 — stock reverse speed (FINAL)

Task S122. 2026-10-27.

## What I did

- `stock::BIKE` (`crates/game-sim/src/stock.rs`): `ReverseSpeed = 4` → `-10`, the value P72's journal
  read off every disc bike. With +4 the reverse push `gain × (ReverseSpeed − speed)` points forwards
  from a standstill, so the stock bike "reversed" forwards.
- Added two lines to the module comment saying the text now differs from
  `tests/freeride-bike.phy.txt` on that one line, and why.
- Re-baselined `crates/game-sim/tests/platforms.rs`. The canonical run's random buttons include gas+brake
  held together at low speed, which is the reverse, so its trace moved from tick 16
  (`bike.pos[1]` -0.12241924 → -0.122292995). I regenerated with
  `cargo test -p game-sim --test platforms -- --ignored --nocapture print_the` and spliced the consts
  in by name, so the hand-written comments between them stayed. Moved: `HASHES`, `TRACE_DIGEST`,
  `POS_Y`, `VEL_Y`, `FINAL_DUMP_HEAD`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`, `RIDERS_HASHES`,
  `RIDERS_DIGEST`, `RIDERS_WIRE_DIGEST`. Unchanged: `FINAL_DUMP_LEAVES`, `EVERY_LEAF_HASH`,
  `RIDERS_WIRE_LEN`, `HUNDRED_TICKS` (lib.rs / hash.rs). So only values changed; the state type did
  not. No other test moved (`./kit check -- -p game-sim --no-fail-fast`: only `--test platforms` was red).
- Nothing was taken from opentennis: it has no bike `.phy`.

## What the original does

See P72 (`research/journal/2026-10-25-p72-reverse-l3`): the push is `./kit fn 0x00257a00`,
gain `+0x548` × (ReverseSpeed `+0x544` − speed), applied only with the rear wheel down. ReverseSpeed
is the `.phy`'s mph × 1.46666, and every disc bike says −10.

## How it was proved

Nothing new needed proving. The stock bike is not the game's (its module says so), and the reverse push
was already ported and proved under P72. This task only gives the stand-in the disc's sign and magnitude.
The re-baselined vectors are this machine's output; the full gate `./kit check --all` is green, with
load flakes passing on a lone rerun.

## Not verified / not 1:1

- I did not re-read −10 off the disc myself; it comes from P72's journal. This worktree's context/ has
  no extracted `.phy` text, and the value only feeds a stand-in bike, so a re-read would prove nothing
  about the port.
- `tests/freeride-bike.phy.txt` and `tests/trick-bike.phy.txt` still say `ReverseSpeed = 4`. The task
  names only `stock::BIKE`, and changing the fixtures would re-baseline tests that are outside it. P72
  patches its test bike to −10 where reverse matters.
- I could not cross-architecture check the new `platforms.rs` vectors: there is no upstream toolchain
  here (see "Cross-architecture runs need an upstream toolchain").
- `fixed_rate` and `smooth_picture` were red in the full gate at load ~36 and passed alone; both are
  known wall-clock flakes and do not ride the stock bike's reverse.

## Next

none
