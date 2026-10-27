# 1 — 2p wrecks (FINAL)

Task S155. 2026-10-27.

## What I did

- `the_2p_wrecks_are_the_originals` in crates/game-sim/tests/ragdoll_wrecks.rs: C10's 17 wrecks and
  29 near misses of the 2p recording (research/c10/events.json), both riders. From a RAM image a few
  ticks before each one, the original's own `bike_vehicle_update` (`./kit fn 0x002520d0`) runs that
  rider's bike tick by tick to the event and one tick past it. Every time the original enters its
  wreck test (`./kit fn 0x002522c0`), the test reads the inputs off the bike. After the original's
  test returns, it hands those inputs to the port's `air::wreck_test` and `air::hit_wreck`, composed
  as rider.rs composes them (the landing clauses only while +0x748 is 0, the hit loop always). It
  records what the original entered through the vtable's "came off" slot (vt +0xfc, a1 = reason).
- Saved 51 new states with kit copy 3 (`research/c10/play_at.sh`, NominalScalar 0.25 for the
  playback, back to 1.0 after): 41 three to four ticks before each event, plus 10 more five to six
  ticks before the five events whose first image was unusable (below).
  research/s155/rams.py turns them into context/states/s155/c10-<tick>.ram. That is C10's rams.py
  pointed at copy 3 and a new folder.

## What the original does

- The wreck test runs once per tick at the end of the bike's vehicle update, which the ride machine
  calls (`./kit xref 0x002522c0`). A wreck is entered through vt +0xfc on the bike with the reason
  in a1. The reason word +0x748 is set from that call on.
- **The tick that writes capture row N steps on the dt written in row N−1.** A capture row's dt is
  the one the next frame takes. If you step each tick on its own row's dt, most runs leave the
  capture's chassis centre within 0–2 ticks. With the row before's dt, the original runs bit for
  bit up to and past the event far more often: 22 of the 46 events (the same probe gave 0–2 ticks
  on most runs before). This matters for any later replay of
  the 2p capture's dt column. C10's 2p dt varies between 0.016667 and 0.016700.
- Some images were saved partway through P1's (or P2's) update: that bike's chassis centre sits
  exactly halfway between two capture rows. These images can't be used for that rider (no register
  state partway through a function), so `row_of` rejects them and earlier images were saved instead.
  Affected: 129120/129482/130414/136137/136139 for P2, 132605/132993/133309/134311/135192 for P1.
- Even from a clean image, a bike-only run sometimes parts from the capture on its first tick. In
  some of those runs the original moves the bike exactly half a tick (one first tick lands halfway
  between rows). In others it parts by a small amount (≈1e-3 ft). The frame does work on the bike
  outside `bike_vehicle_update`. The player update (`./kit fn 0x0027c890`, which calls it) made no
  difference. The bike's update draws nothing from `rand` (rand_state is unchanged over 8 updates).
  The two Scenery wrecks are where this shows most: from P2's images the run parts on the hit tick,
  and the original never enters reason 6. The frame's split is not known; filed as S174.

## How it was proved

A RAM-image run of the original's own function with hooks: `the_2p_wrecks_are_the_originals`, 46
runs over C10's index (17 wrecks, 29 near misses), from context/states/{c10,s155}.

- **Port == original on every wreck-test call of every run** (several hundred calls). That includes
  the 12 runs whose original enters its wreck after leaving the capture. Each of those enters the
  indexed reason, on the indexed row or one row early: 129945/8, 132997/3, 133313/12, 133800/8 and
  140363/2 on the row; 135196, 135855, 136559, 139659 and 134315 a row early. The port enters the same
  reason on the same call every time. The port's composition also gives an empty answer on every call
  where the original enters nothing.
- **22 events run on the capture's own line**: the chassis centre matches bit for bit from the image
  through the event row. Five wrecks (P1 128995 NoseDive, 130966 Tail, 133953 and 134381 NoHander;
  P2 131595 Front) are the original's first wreck, on the indexed row, with the indexed reason. 17
  near misses are ridden out through the row after, with the clause's quantity equal to C10's index
  (within 0.0015°/g). The floor is `TWO_P_ON_THE_CAPTURE = 22`.
- Mutations, each failing the test, then put back: the floor at 23 ("only 22 of the 2p events ran
  on the capture's own line"); the port's landing clauses switched off ("P1 128995: row 128995: the
  port's wreck test answers [], the original's [8]").

## Not verified / not 1:1

- **24 of the 46 events are not reproduced on the capture's line** (listed by the test's output):
  12 wrecks, 129945, 132997, 133313, 135196, 135855, 136559 and 139659 (P1) and 130418, 130897,
  133800, 134315 and 140363 (P2), and 12 near misses, among them S156's 131484 TouchRoll (85.90°
  here, the run having left the capture a tick before). On these, only port == original per call is
  proven, not "same tick, same reason as the recording". **P2's two Scenery wrecks (130418, 130897)
  are not entered by the original's run at all,** so the port's scenery hit path is not checked
  against this ride. It is proven on generated states in air.rs's `the_hits_are_the_originals`.
  Reason: the bike-only run (above). Unblock: S174, the whole simulation step from each image.
- **The port's ride is not run from the 2p images.** Only its wreck decision is checked: on the
  original's own inputs, call by call. Whether the port's bike reaches those inputs (pitch, roll, g
  loads, contacts) on its own is S67-style replay work (`SaveState::seed_ride` in replay.rs). That
  path does not seat the Race500P on Free09_SS from a 2p image yet. Filed as S175.
- Bools in the inputs are read from the bike as words (`!= 0`). The original reads them as bytes or
  words in different places. A stray high byte would read differently, but none showed up.
- The 2p ride has no rider-vs-rider, truck or prop contact (C10), so `hit_wreck`'s joust branch
  (reason 7) has nothing to be checked against here. Its scenery branch would have been checked
  only through P2's two Scenery wrecks, which the runs don't reach (above).
- wreck_page.rs and air.rs are untouched: the port agreed with the original on every call, so no
  finding against them came out of this. wreck_page (the wreck's pages after entry) is not exercised.
  The runs stop a tick after each event.
- The RAM images are in s3's context/ only. Other slots skip the test with a line until they have
  context/states/s155 (research/s155/rams.py rebuilds them from copy 3's saves 20–70).

## Next

S174 (the whole frame from each image, so all 46 run on the capture's line), then S175.
