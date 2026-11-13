# 1 — free09 ai4 capture (FINAL)

Task C33. 2026-11-13.

## What I did

- **research/c33/** (README.md there lists every script and file). The work started from C31's
  scripted boot (research/c31/boot.py); there was nothing to take from opentennis.
  - `boot.py` boots free09 as recording2's event: league 2, kind 5 (Freestyle), 4 racers, +0x114 = 1,
    +0x115 = 5, AI difficulty +0x117 = 90. It saves the state at the first frame past
    frame_counter 1.
  - `keep.py` keeps that state and its RAM beside the csv.
  - `riders.py` finds the riders in it.
  - `spec.py` writes the watch spec at the riders' fixed addresses.
  - `record.sh` records from the kept state.
  - `agree.py` merges recordings of the same ride.
- **Captures** in context/capture/c33 (git-ignored):
  - `free09-ai4.p2s` / `free09-ai4.ee.bin`: the start state, frame_counter 2. All four bikes stand on
    the spawn line (x 4318, 4322, 4310 = human, 4314; z ≈ 3314.5) with zero horizontal velocity.
  - `free09-ai4.riders.json`: rider, body, interface and brain for each rider, in actor order.
  - Three recordings of the same 1800 ticks from that state, all with 0 torn reads and 0 gaps:
    - `free09-ai4.csv`: speed 0.3, the boot run, frames 6..1805. The recorder's 300 ms settle after
      the load cost ticks 3–5.
    - `free09-ai4-slow.csv`: speed 0.1, settle 100 ms, frames 3..1802.
    - `free09-ai4-mid.csv`: speed 0.2, frames 3..1802.
  - **`free09-ai4-agreed.csv`, the one to replay.** For each frame it holds the row most recordings
    hold, plus `agree`/`of` columns:
    - 1068 rows are held by all three;
    - 709 rows by two;
    - 26 rows by one only (`agree` = 1): 23 inside the ride, plus 1803–1805, which only the boot run
      has.
- **Columns, per tick:**
  - `frame_counter`, `dt`, `rng_state` (@rand_state) and `in_game`.
  - Per rider (`ai0_`..`ai2_` in actor order, plus `human_`), from the body (rider +0x7a4):
    - centre +0x60;
    - quat +0x1a0;
    - world velocity +0x260;
    - world spin +0x280;
    - body-frame spin +0x290;
    - forward speed +0x278;
    - the raw first 0x2c0 bytes.
  - Also per rider: the rider's on-ground word, +0x574.
- **The riders:**
  - AI rider 0x005ff210: body 0x00899860, brain 0x008a8800.
  - AI rider 0x008c7b10: body 0x008cf9b0, brain 0x00888360.
  - The human, rider 0x00888770: body 0x0088cfc0.
  - AI rider 0x0149c910: body 0x009014b0, brain 0x009453b0.
- **The ride:**
  - The three AI riders leave the line around tick 10–16 and ride at 40–70 ft/s for the rest of the
    capture.
  - The human's bike has no brain and no input. It rolls slowly down the spawn slope and then runs
    away downhill from about tick 790, reaching 53 ft/s with no AI rider within 70 ft. So the human's
    body is part of what a replay has to step.

## What the original does

- **The Freestyle loading screen.** It never finishes with all_ai 0: frame_counter stays at 0 or 1 and
  the bar is half full. It probably waits for a press, which no pad can give here. With all_ai = 1 it
  loads. The human's rider still gets no brain (its interface +0 is 0), as C31 found.
- **Bike class.** With the event's bike class +0x10c = 1 (recording2's value), the load stalled twice
  with the bar a quarter full. With 2 it loads.
- **frame_counter during the load.** frame_counter stays at 0 and then at 1 while the world loads after
  in_game turns 1. A state saved at 1 never runs on once loaded. The riders, their bodies and their
  brains already exist at frame 0.
- **The actor list.** An actor's first word is not a type: it was 2 on recording2's riders and is 1
  here. The riders are found through their interface instead (vtable 0x004550e4 at +8, the rider at
  +0xc, the brain at +0, or 0 for the human's).
- All of this is noted in research/oracle.txt (C33 block).

## How it was proved

- **Repeatability.** The three recordings were made at three emulator speeds from the same state.
  - They end on the same positions, so a ride from the state repeats.
  - Pairs of recordings differ on 258–635 of their ~1800 rows. The differing rows are reads that
    landed mid-tick, where one rider's fields are half-updated. They are not divergence: the rows after
    them agree again.
- **Picking the clean rows.** Two-out-of-three agreement picks each row.
- **Cross-check.** Compared against velocity × dt, the agreed csv has the fewest position-step
  outliers of the four: 156 steps off by more than 0.5 ft, against 277–664 in the single runs. The rest
  are probably substeps and contacts, which position does not follow as velocity × dt.

## Not verified / not 1:1

- **Rows with `agree` = 2 may still be torn.** C32 found that some mid-frame reads repeat at every
  speed, because PINE reads at a fixed point of emulated time. A row that two recordings tore the same
  way would pass as clean. Nothing here can tell it apart; S12e's replay will show it as a one-row
  mismatch that does not carry on.
- **26 rows have no majority** (`agree` = 1). The replay should not compare them.
- **Ticks 1–2 are not recorded.** The state is taken at frame_counter 2. Its RAM image holds tick 2,
  and the first row is frame 3.
- **The event is not recording2's exactly.**
  - Bike class is 2, not 1, because 1 stalled the load.
  - all_ai is 1, not 0, because 0 never finishes loading.
  - The track name is written in lower case, `free09`, where the menu writes `Free09`.
  - All of all_ai's effects are not known. It is read gp-relative, so `./kit xref` finds no reader. C31
    saw it change nothing on the bikes, but that is not proved for free09's AI.
- **The brains are not recorded.** The task asks for the bodies and rand_state only. A replay that
  drifts will have to find out why from the bodies alone, or record the brains in a new capture made
  with `record.sh` from the kept state.
- **The `frame` column.** It is the recorder's own count. Use `frame_counter`.

## Next

S12e (after S322 and S323) can replay `free09-ai4-agreed.csv`:
1. Seed from `free09-ai4.ee.bin`.
2. Step the four riders tick by tick.
3. Check `rng_state` and each rider's body on every row with `agree` ≥ 2.
