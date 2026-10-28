# 1 — 2p port wrecks (FINAL)

Task S175. 2026-10-28.

## What I did

- `crates/game-sim/tests/replay.rs`, new section after the 2p recording test:
  `the_ports_own_2p_ride_wrecks_where_the_original_did`. For each of C10's 17 wrecks and 24
  near-miss rows (the 29 near-miss entries of `research/c10/events.json`, folded to one per row
  and rider), it looks for the latest c10-<t>.ram before the event, in `states/c10` or
  `states/s155`. It places the image on its capture row the same way S155 does: the event rider's
  chassis centre and wreck reason, bit for bit, within ±2 of the image's frame counter. Both riders
  are seated on the disc's Race500P over free09 with the bike's body rays. Each is seeded off its own
  bike, then two players ride the capture's two pads up to the event row and one past it. Each tick
  uses the row before's pad and dt (lag 2, as S67/S138 do).
- `SaveState::seed_2p_rider`: `seed_ride` plus what a 2p wreck reads and `seed_ride` leaves at its
  defaults:
  - the backflip (`seed_flip`);
  - the trick machine: current/requested trick, restart, seq/prev seq, elapsed, progress, the
    hold's three clocks and its done flag, in-progress, safe, the no-hander, the gesture's armed flag
    (offsets from `tests/trick.rs`);
  - the animation driver: shown crossup, pose, fore-aft, faded, ramp (`tests/motion.rs`).

  Without them the first run gave 26 of 41. The port's `shown_crossup` defaults to 0, so every
  landing in the window was judged crossed up to the right (reason 3). Unseeded trick state missed
  all 7 no-hander wrecks.
- Nothing in `src/` changed: no finding against the port's ride or its wreck test came out.

Nothing was taken from opentennis. It has no rider-wreck replay.

## What the original does

The capture's rows tear around these events, so "the same tick" cannot be read off the
`wreck_reason` column alone. Rows that do this:

- **Stalled rows.** A row repeats the row before's chassis centre bit for bit, and the next row
  steps twice: 133310 = 133311, 135852 = 135853, 135191 = 135192, 130895 = 130896. 139657/139658
  differ by only 0.002 ft. Every row's dt is the ordinary 1/60 (0.0166667 / 0.0167), so the game did
  not drop a frame. The recorder read the centre at another phase of the frame.
- **Reason a row late.** The bike stands still a row before its reason shows: 135195, 136558,
  134314. Elsewhere the reason row is the first still row (128995, 131595, 133800).

Where the port enters its wreck a row early, its chassis on that tick is the original's on the row
that ends the motion, to 0.003–0.03 ft. At 133313 the port's centre on row 133311 is the capture's
to 0.01 ft, and the capture's next row is two steps further on. Its 0.78 ft "off on the row" is half
of that double step. S155's run of the original's own bike update, from these same images, also
wrecks a row early on 135196, 135855, 136559, 139659 and 134315.

## How it was proved

A seeded-ride replay against the 2p capture of record (C10's `context/capture/2p/2p.csv`) and the
RAM images in s3's `context/states/{c10,s155}`. The rule is S138's: the same reason, on the same row
or one either side. Both riders' first wrecks in each run are held to the capture's.

- **39 of 41 events agree. 33 are on the row itself** (floor `S175_ON_THE_ROW = 33`), and none part
  outside the known list:
  - 15 of the 17 wrecks: P1's 11 (NoseDive ×2, Tail, Right, NoHander ×6, Left) and P2's Front,
    NoseDive, NoHander and Left. Nine are on the row: 128995, 129945, 130966, 132997, 133953,
    134381, and P2's 131595, 133800, 140363. Six are a row early, per the tear above: 133313,
    135196, 135855, 136559, 139659 and P2's 134315.
  - All 24 near-miss rows ridden through the row after by both riders, among them S156's 131484
    TouchRoll.
- **Mutation:** dropping the `shown_crossup` seed fails the test (30 of 41, 9 parted). It was put
  back and the test passes.
- Pad lag 1 and 3 were both tried. Lag 1 puts 139659 on its row and changes nothing else. Lag 3
  changes nothing. Lag 2 stays: it is S67/S138's, and `two_p::POLLED_FIELD_LAG` reads the same way.

## Not verified / not 1:1

- **P2's two Scenery wrecks (130418, 130897) are not entered by the port.** They are listed in
  `S175_KNOWN`. S155's run of the original's own bike update does not enter them either. The trunk
  hit needs the composed wreck list over a whole frame from the image, which is S174. They are
  unchecked here.
- **"Same tick" is ±1 row, not exact.** On 6 of the 15 agreeing wrecks the port is a row early. The
  evidence above says these are torn capture rows and not port timing. The proof would be the
  original's whole frame run from each image (S176), and that is not done.
- **A rider the image holds down from an earlier wreck is not judged.** That is P2 in 5 of P1's runs
  (130966, 133953, 134381, 130420, 131628). `seed_2p_rider` seats a riding bike; the wreck in
  progress (the wreck body, ragdoll and page) is not seeded. In those runs the port's P2 re-wrecks
  (Left at 133951, NoHander at 134379), which the original's still-down rider cannot.
- **Not seeded:** the trick queue (a heap list at the bike's `+0xa38`), the gesture's group and
  direction (recomputed from the pad each frame), the hand-hold pose, and the input layer's previous
  pad state. On tick 1 a held button reads as a fresh press to the port. For these events that
  changed nothing measurable.
- **Runs are 1–5 ticks long.** They prove the wreck decision on the port's own physics from the
  original's state. They do not prove a long ride into the wreck: the images sit 1–5 ticks before
  each event.
- The RAM images are in s3's `context/` (and s1/s2/s5). A slot without them skips the test, and a
  slot missing only some of the images prints the missing ones and returns before the verdict.
- No rider-vs-rider, truck or prop contact happens in the 2p ride (C10), so the joust branch is
  not exercised.

## Next

S174 (P2's trunk from a 2p image) would take the two `S175_KNOWN` entries out. S176 (the original's
whole frame from each image) would show whether the row-early six are the capture's or the port's.
