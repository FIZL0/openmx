# 1 — atv clutch lever (FINAL)

Task MD178. 2026-11-16.

## What I did

- `game_sim::quad::Ride::controls` now sets the quad's clutch lever (`Bike::clutch`) from the pad's
  clutch, as a switch (`ride::switch`: 1.0 for any strength but zero, 0 for zero). Before, it stayed 0.
- The input limiter, the clutch ramp and MD162's engine note all read that word, so a held L1 now
  reaches them.
- Wrecked quads already get let-go controls from `Ride::step` (`down()`), which matches the
  original's wrecked branch (lever 0).
- Proof: `crates/game-sim/tests/atv_quad/clutch_lever.rs`, mounted in `tests/atv_quad.rs`.
- Filed MD178a for the lever's two other writers (race-start gate, vehicle states 3/4).
- opentennis: nothing to take (no clutch, no pad-lever switch of this kind).

## What the original does

- The quad's pad reader is `./kit fn 0x004e36c0 -o atv`. The quad's vtable 0x003ce250 holds it at
  +0x4cc, and its thunk 0x004bfa80 at +0x1c0. So it is the quad's own reader, not only the bike's.
- Gated on +0xbc4 and the vehicle being live.
- Wrecked (+0xc80): stick, gas, brake, clutch lever +0x12c0 and +0x14d4 zeroed.
- Else command 0x10 (L1) through the binding lookup: strength != 0 → +0x12c0 = 1.0, else 0.
- On a fresh press (latch +0x17ac != 1.0) it posts a listeners' message (`./kit fn 0x00272300 -o atv`,
  args 8, 0xf: the press cue), then copies the lever into the latch. Let go clears the latch.
- Other writers of +0x12c0 (from a full disassembly of the game overlay, grep of stores at 4800):
  the race-start step `0x004e1460` (gate flags → 1.0 or 0), the vehicle update `0x004e1b90`
  (states 3/4 → 0), the bike reset `0x004de160` (→ 0), plus non-quad classes (`0x004c3d60` biplane,
  `0x004cd900`, `0x004c72c0`). Listed in research/oracle.txt `# MD178`.

## How it was proved

- Function oracle, 10,000 generated cases: the original reader run in the interpreter on
  `context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`, its own binding table answering the lookup.
- Each case: player 0/1, every control slot's strength (±0, 1e-30, 1.0, part presses, negatives),
  wrecked on/off, the lever and latch's old words random. The game's +0x12c0 must equal the port's
  `Bike::clutch` bit for bit; the press cue must fire exactly on a fresh press.
- Each path (wrecked, let go, let go -0, held, held tiny, held negative) taken at least 100 times.
- Mutant check: dropping the switch (lever = raw strength) fails at case 0. Restored.

## Not verified / not 1:1

- The press cue (the listeners' message on a fresh L1 press, and its latch +0x17ac) is not ported.
  It is a presentation message, not a ride word: only the reader reads the latch. The test proves
  when it fires, but nothing in the port sends it.
- The race-start gate's clutch hold and the vehicle update's states-3/4 clear are not ported (MD178a).
  They are other writers of the same word, outside the pad read.
- Not checked on a live capture with L1 held: no PCSX2 run. The oracle covers the reader, but not
  what the tick after does with the lever in the original.
- "Every quad replay hash unchanged" is checked only through the test gate (`./kit check --all`).
  I did not check whether any quad tape holds L1; a tape that never does would not exercise the new path.

## Next

Done when `./kit done` lands; MD178a carries the two other writers.
