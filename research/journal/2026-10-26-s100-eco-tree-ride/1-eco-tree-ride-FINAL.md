# 1 — eco tree ride (FINAL)

Task S100. 2026-10-26.

## What I did

- Rode the original's 125 flat out into a free09 trunk in PCSX2 (copy 2), twice, and captured both rides
  (`capture/` holds the scripts; the captures are `context/capture/s100/tree.csv` and `nowreck.csv` in
  s2, with `context/s100/` holding the states and tapes):
  1. `survey.py` loads recording2's state into scratch slot 8 and lists free09's trunked plants by
     distance. The bike faces +z (its transform's row 2 is (−0.001, −0.002, 1.000)).
  2. I chose plant 347, a TreeB at (4968.84, 180.35, 3087.89). It stands in the open, 60 ft of clear
     ground south of it. `tele.py` (P21's `tele2.py` with S89's
     braked settle) moved the bike 60 ft short of it, then settled and saved it to slot 9. Each settle
     turned the bike about 0.45° to the left, so I teleported three times, shifting x each time. The
     last start, at cofg (4971.13, 181.84, 3027.55) facing (−0.044, 0.006, 0.999), heads 0.44 ft off
     the trunk's centre. The ground there is flat: the pitch component is 0.006, and the rows ride at
     cofg y 182.09 all the way to the tree.
  3. `prep.py` copies the state and writes a full-Cross tape with P21's `make_p2m2.py`. `capture.sh`
     (P21's `flat_capture.sh`, with its work dir under `context/s100`) plays it with `extra.toml`'s
     fields added: the chassis' cofg and velocity, the throttle, and the bike's collision record
     (+0x7d0: touched, count, and the first four list entries' kind, ray end, normal, point and other
     actor).
  4. Run 1 (`tree.csv`) is the state as it is, with 300 fields of tape. Run 2 (`nowreck.csv`) is
     `nowreck.py`'s copy of the state, with the original's `wrecks_enabled` word cleared and saved to
     slot 8, and 600 fields of tape.
- `crates/game-sim/tests/eco.rs` `a_free09_tree_against_the_original` (new) reads both captures and
  checks the original's facts and the port's ride beside them (below). It skips with a line where the
  captures, the disc or the kit are missing.
- `riding_into_a_free09_tree_stops_the_bike_at_its_trunk`: I dropped its "no wreck" assert. The
  original *does* wreck on this impact, so the assert pinned a behaviour the original does not have.
  A comment now points at the new test.
- `research/oracle.txt`: an S100 block with the live record layout, the wrecks-off trick and the tape
  rate.
- Filed **S124** (the port's trunk contact makes no impact load, so there is no wreck and it rides on
  past the tree) and **P154** (the 125's launch from rest is quicker than the original's).

opentennis: nothing there for this; the PCSX2 side reuses this repo's own C6/P21/S89 scripts.

## What the original does

Both runs are the same ride up to the trunk, from the same state and tape. Where both read a frame
at the same phase, the rows are identical (e.g. frame 2971, 52.15 ft/s at z 3073.66). Elsewhere a
row can sit a phase apart: at row 10 one run reads 8.91 ft/s and the other 7.95.

- **The stop tick.** Frame 2977, tick 61 from the state. Before it the chassis does 58.16 ft/s with
  its cofg 3.13 ft from the trunk's axis. With wrecks off, that tick takes the bike to (−7.29, −0.02,
  34.85) ft/s, 35.6 ft/s level.
- **The contact.** Two of the bike's own body rays find the trunk: list entries of kind 0 and
  other 0 (scenery), from the rays ending at (0, 3.1, 3.0) and (0, 1.9, 3.6) in model space (the
  front ones). Both report the normal (−0.1959, 0, −0.9806). That is the 16-gon's facet that faces
  the bike, at 258.75°. The points lie on that facet's plane.
- **The wreck.** With wrecks on, the same tick (frame 2978, the row phase being ±1) enters wreck 1
  (`Wreck::Front`, over the bars) with g_peak (−9.19, 14.22, −28.94). That is the fore-aft
  impact-load clause. The chassis then freezes at its last values while the wreck runs. The rider is
  put back between rows 140 and 150, some 80–88 ticks later, at (4907.5, 125.9, 3128.3), some 73 ft away and 55 ft lower.
- **Under gas after the hit (wrecks off).** The bike rears up against the trunk and the cofg climbs
  to about 185.2, and by tick 80 the bike is all but stopped (0.28 ft/s). Ground contacts (normal
  (0, 1, 0) at y 180.37) hold it while it stands up, and side contacts come from the trunk's +x facets as it slides round. By tick
  ~100 it faces back (−0.27, −0.96) and rides away along −z, down a slope off the flat. At tick 299 it
  is 376 ft from the trunk at 55.7 ft/s level, with Cross held the whole way.
- **The launch.** 8.9 / 18.5 / 28.0 / 37.3 / 47.5 ft/s at ticks 10 / 20 / 30 / 40 / 50: an even
  ~0.95 ft/s a tick. P21's lane capture agrees (28.8 at 1 s).
- **The tape.** A generated .p2m2 played *two fields a tick* here. Run 1's 300 fields held Cross from
  row 1 to row 150, and run 2's 600 fields held it for all 300 rows.

## How it was proved

Two live PINE captures (300 rows each, one a tick) compared with the port's same ride, all in
`a_free09_tree_against_the_original`:

- **Matches, asserted:**
  - The trunk is where the port puts it and is the shape the port builds. Each of the original's
    contact normals is a facet normal of `eco::cylinder` for plant 347's solid (cos ≥ 0.99999; it reads
    1.000000). Each contact point is within 0.01 ft of that facet's plane; it reads 0.0002–0.0003 ft.
    This adds a live check to S97's save-state one.
  - Both games stop the bike on the trunk, nose first, in one tick. The original's cofg is 3.13 ft
    off the axis; the port's is 2.66 ft off, with the test's band at 2.5–4.5.
- **The original's own facts, asserted from the captures:** wreck 1 on the tick the rays find the
  trunk, a fore-aft g_peak below −25, and with wrecks off a bike going back along −z at the end.
- **Differences, printed, not asserted** (each now has a task):
  - The port hits at 64.62 ft/s (tick 51) where the original hits at 58.16 (tick 61), because of the
    quicker launch (P154).
  - The port stops much harder, 64.6 → 4.2 against 58.2 → 35.6.
  - The port's g_peak is 0, so it never wrecks.
  - Afterwards the port creeps on and ends 683 ft *past* the tree at 115 ft/s. The original wrecks,
    or with wrecks off it turns back (S124).
- `./kit check -- -p game-sim --test eco`: 6 tests pass. I forced one run of the new test to fail so I
  could read its output and confirm it ran rather than skipped.

## Not verified / not 1:1

- **The port's contact normal is not compared.** The port's trunk push happens inside its frame
  probes and the record is not exposed, so only the original's normal is checked (against the port's
  trunk *geometry*). Which port probe takes the trunk, and with what normal, is unknown.
- **The port does not match the impact (S124).** No impact load, no wreck 1, and after the stop it
  rides past the tree instead of rearing up and coming round. The test prints these and does not
  assert them.
- **The port's launch is quicker (P154).** So the port's stop is at a different tick (51 against 61)
  and speed (64.6 against 58.2), and the "2.66 against 3.13 ft off the axis" comparison is at
  different speeds.
- **The port rides a Flat floor at the plant's foot (180.35), not free09's terrain.** The original's
  ground under the run looks flat (pitch 0.006, cofg y 182.09 throughout), but I did not load the
  real heightfield, and its surface id is not known. The port's start is `Rider::new` at the
  capture's cofg x, z and heading, not the captured state (wheels, suspension, engine).
- **The original's start.** It was teleported three times and settled braked, so the original also
  starts with a settled but not a fresh state. Its heading turned about 0.45° at each settle; the run
  passes 0.44 ft off the trunk's centre, so the hit is not exactly square.
- **Which wreck clause fired is inferred.** I read wreck 1 and g_peak.z = −28.94 and matched them to
  the fore-aft clause in `air::wreck_test`. I did not read the 125's FrontWreckSpeedLimit off the
  disc to confirm that −28.94 is past it.
- **Rows read at mixed phases.** The wreck run repeats rows (56/57, 59/60) and its wreck shows on the
  frame after the no-wreck run's contact. I compare by frame within ±1, not by row.
- **The put-back after the wreck** (73 ft away, 55 ft lower) was not compared with the port's
  put-back.
- **Only one tree, one approach, one bike.** That is one TreeB, head-on, with the 125. No glancing hit
  and no other species. Each species has its own solid radius; only TreeB's 1.50 is checked.
- **The memory note "a .p2m2 frame is a game tick" disagrees with this run** (two fields a tick). I
  did not find out why the animandpump recording behaved differently.

## Next

S124 (the trunk impact through the body rays' resolve) and P154 (the launch).
