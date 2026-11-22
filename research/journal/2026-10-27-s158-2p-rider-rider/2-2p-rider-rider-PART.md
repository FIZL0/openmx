# 2 — 2p rider rider (PART)

Task S158. 2026-11-21. A new 2p capture with a rider-vs-rider contact, made from a generated two-pad tape
(no person), and its contact replayed bit for bit as S88's.

## What I did

- **The tape.** `research/s158/tape2.py` (R22's tape2.py at one field a tick, as the 2p state reads)
  writes a two-pad `.p2m2` from two pad scripts, and `research/s158/make.sh` puts the 2p state beside it:
  `bash research/s158/make.sh research/s158/gas.pad research/s158/p2_rear.pad rear` →
  `context/recordings/split/s158/rear.p2m2`. P1 holds the gas straight on (`gas.pad`); P2
  (`p2_rear.pad`, from `research/s158/rear.sh 293 50 0.4 10 0.86`) steers 0.86 right for 10 ticks at
  tick 280 to line up behind P1, brakes for 50, then rides on at 0.4 gas into P1's back.
- **Scouting.** `research/s158/scout.sh` plays a tape on a kit copy and `watch.py` prints both riders
  over PINE (distance, speed, wreck). About twenty steer/brake/gas variants were scouted that way; the
  rear one is the one that met. Only its pads are kept.
- **The capture.** C10's recipe at NominalScalar 0.05:
  `./kit pcsx2 with -- bash research/c10/play_capture.sh PID context/recordings/split/s158/rear.p2m2 127882 570 context/capture/2p/s158-rear.csv`
  (570 rows, none lost). `python3 -I research/s158/contacts.py context/capture/2p/s158-rear.csv` lists
  the rider-vs-rider rows.
- **The images.** C10's `play_at.sh` saved states at frames 128372, 128374, 128376–128381, 128391,
  128392 and 128413; `python3 -B research/s158/rams.py rear LOG` turns them into
  `context/states/s158/rear-<frame>.ram`, each named by the frame_counter it holds.
- **The tests** (`crates/game-sim/tests/rider_rider_capture.rs`):
  - a `State` per recording (node, players, phy, row lag, resolves a tick): S93's split state and C10's
    2p state; every S88 test now takes the state from its contact;
  - the rear strike's 128378 is a third named contact, through all four of S88's tests (original tick,
    the port's `gather`, the port's `Bump::resolve` on every rider call, the tick with the port's
    resolve) and the order test;
  - `the_rear_strikes_images_are_one_ride`: each image ticks into the next to the bit (chassis, lists,
    wreck reasons) and each sits a row on in the capture;
  - `the_original_tick_into_the_wreck_is_row_128380`: P2 riding into P1's wreck, as S88's 3414.
- **S88's tests had been skipping on main.** S93's images and captures moved to `context/states/s93/`
  when the contexts were merged; the tests looked only in the old places and skipped, which a pass hides.
  They now look in both. Four of S93's images and `splitscreen.ram` are not in the merged context at all,
  so those cases still skip (see below).

## What the original does

- The 2p state ticks at 60 Hz with one substep a tick, dt alternating 0.0166999 and 0.0166667. Each
  rider resolves its list once a tick; the node updates P2's vehicle before P1's, as in the split state.
- The recorder reads a row as the frame starts, before the simulation, so row f+1 holds the end of frame
  f (image f). Two rows were read later, inside the frame:
  - row 128377: after the node's update of P2 and before P1's (P2 is image 128377's, P1 image 128376's);
  - row 128378: after the search (its lists are the contact's) and with P2's force and moment half built.
- **The contact, frame 128378.** One of P1's body rays finds P2's frame: P1's list holds
  `(0, P2's object)`, P2's the mirror `(1, P1's object)`. P1 is thrown off the back (wreck reason 4,
  `Wreck::Back`, the fore-aft load); P2 drops from about 70 to about 36 ft/s and rides on.
- **Into the wreck, 128379 to about 128428.** P1's riding object leaves the node and the wreck bike's
  object takes its place. P2's ray strikes P1's frozen frame, and the wreck bike's body hits P2 twice:
  P2's list is `(0, P1's object), (1, wreck bike), (1, wreck bike)`. P1's frozen object is never emptied
  again, so it gathers the mirror of P2's ray each tick until its 11 entries are full. From 128392 P2's
  ray finds the wreck bike itself, `(0, wreck bike)`. This is S88's 3414 again, on a Race500P.

## How it was proved

Save-state capture and bit-exact replay in the interpreter (`cargo test -p game-sim --test
rider_rider_capture`, 8 tests):

- image 128377 through the original's collision node on the next frame's dt is row 128379 (image 128378)
  for both chassis, all seven quantities, and P1 wrecks with reason 4;
- the port's `rider_contact::gather` on both frames gives the original's search, every field, and the
  row's entries;
- every rider resolve in the tick (P2's, then P1's) gives the same bits from the port's
  `Bump::resolve` on the same RAM, both chassis;
- with the port's resolve standing in, the tick still lands on the row and P1 still wrecks with reason 4;
- the order test: the node resolves P2 before P1 in the 2p state too;
- the nine images 128376–128381, 128391, 128392 and 128413 against the capture, and the five
  consecutive pairs ticking into each other, chassis, lists and wreck reasons;
- 128378 ticked is row 128380, with P2's three entries and P1's two frozen mirrors.

No change to `rider_contact.rs` was needed: the port already matches on a Race500P.

## Not verified / not 1:1

- **P2 riding into P1's wreck is the original's side only.** The port has no wreck bike and puts a
  downed rider in no list (S88, S105), so P2's strikes from 128379 on are pinned in the original only,
  as S88 did for 3414.
- **The port's own wreck test is not run on the contact.** The tests prove the original's tick wrecks
  P1 with reason 4 when the port's resolve stands in; the port's `air::wreck_test` on its own loads is
  not checked against it, as in S88.
- **Only one contact, a rear strike.** Head-on, side swipe and landing on the other rider are not
  recorded. The brief's "every 2p contact" holds for the recordings there are: C10's own 2p ride has none
  (part 1), and this one has one.
- **Rows 128377 and 128378 are torn.** The recorder read them inside the frame. The tests leave out what
  the tear changed (128377's P2, 128378's P2 force and moment) and prove 128377's P2 against the next
  image instead.
- **Four of S93's images and `splitscreen.ram` are missing** from the merged context (s93-long-3413,
  -4661, -5074, -5505), so S88's 3413 case and four order-test images skip. Filed as a follow-up.

## Next

Spec review, then `./kit done`.
