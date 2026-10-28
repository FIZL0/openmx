# 1 — s89 swept contact point (FINAL)

Task S123. 2026-10-27.

## What I did

- **Ee-stepped the original's ground step from `context/states/s89-along.eeMemory.bin`** for one frame
  (`./kit fn 0x00257a00`, bike = the S89 rider, f12 = the capture's dt 0.03333). I used S107's method in a
  temporary block in `the_dallas_rail_rides_replay_from_their_save_states`, removed before the commit:
  - a hook on `wheel_set_contact` (`./kit fn 0x0025b250`) logged the point and normal it was handed and the
    wheel probe's whole record (bike `+0x7c8`, record `+0x6c`: rays `+0x90`, points `+0x98`, fractions
    `+0xa8`, objects `+0x9c`), then did its five stores itself;
  - a hook on `wheel_tire_forces` (`./kit fn 0x0025bc70`) logged the wheel's `+0xd0`/`+0xd4`, contact
    point `+0x120` and normal `+0x130` going in, ran the function as a nested call (`ee.pc` reset, see
    the ee-hook memory), and logged contact, load point `+0x140` and compression `+0xf0` coming out.

  Beside it I put temporary `eprintln`s in the port's `ride::contacts` (the wheel hits) and
  `wheel::tire_forces` (contact, load point, compression), for one `tick_in` from the same seeded sim.
- **The finding: the parting S107 described is gone on HEAD.** In both substeps of tick 1 the
  original's swept contact on the bed is the ray probe record's point, which `bike_wheel_contacts` hands
  `wheel_set_contact` unchanged. That is the port's hit point, word for word:
  front `[458f7a44 4307eb9b 45356353]` in substep 1 and `[458f7a43 4307eb9b 45356353]` in substep 2;
  rear `[458f7e73 4307ebe9 453511f4]`, then `[458f7e71 …]`. The load points (front `…45356375`), the
  compressions (front `3eec9a08` both substeps, rear `3ee50209` then `3ee50609`) and the normals
  (`-0.0022749244, 0.9999973, -4.0024148e-7`) all agree too. S107's "port contact z `0x45356375`" is the
  **load point's** z. The contact's z is `0x45356353` in both, so the original and the port place it the
  same way. The original's contact is not on the suspension line, so the load point (the line through
  the contact's plane) differs from it, as the tire step computes it.
- **`S89_AGREES_THROUGH` [1, 1] → [30, 15]** (crates/game-sim/tests/replay.rs). The test now says every
  lane agrees through tick 30 along and 15 across. Its doc comment and the bed paragraph's "tick 1 parts"
  comment were updated. `S89_LANES_AGREE_THROUGH` asked for no raise.
- No port code changed: the swept contact's placement was already the original's.
- Followed up: both rides now part at the same place, where the original's engine first drives the rear
  wheel (S183).

Nothing was taken from opentennis: it has no bike or wheel probe.

## What the original does

`bike_wheel_contacts` (`./kit fn 0x00257820`) clears the record, runs the wheel probe's search, and walks
the record's contacts. For each, it passes the contact's own point and normal (`record+0x98`/`+0x94`,
indexed per contact) to `wheel_set_contact`. A contact goes to the front wheel when its ray's `from.z` is
above 0, otherwise to the rear (the rear also keeps the struck node at bike `+0x7d4`).
`wheel_set_contact` (`./kit fn 0x0025b250`) copies the point to `+0x120` and the normal to `+0x130`,
stores owner and vehicle at `+0x1f0`/`+0x1f4`, and sets `+0xd4` = 1. Nothing moves the point: a swept-only
contact is the mesh hit exactly as the ray-against-triangle test and the mesh transform leave it.
`wheel_tire_forces` then crosses the suspension line with the contact's plane to make the load point. It
does not overwrite the contact point.

## How it was proved

- **Ee step against the port, one frame, both substeps** (temporary trace, above). Contact points, load
  points, compressions and normals agree word for word for both wheels on `along`'s tick 1. The record
  holds two contacts, both on object `0x00be8f50` (the bed): front fraction `3f4a3a43` then `3f4a3adc`,
  rear `3f4bf410` then `3f4bf4a3`.
- **Save-state replay ratchet**: `the_dallas_rail_rides_replay_from_their_save_states`, run through
  `./kit check -- -p game-sim --test replay -- --nocapture the_dallas_rail_rides` in s3 (which has
  context/capture/s89 and both s89 states, so the test does not skip). It printed `along now agrees
  through tick 30` and `across now agrees through tick 15` on every lane: chassis, both wheels' load
  point, normal, contact velocity, compression, spring force, spin, engine, slip and contact flags.
  The ratchet is now [30, 15].

## Not verified / not 1:1

- **Which commit fixed tick 2 is not known.** S107 (10-25) and S114 (10-26, after the wheel probe) both
  recorded the whole run parting at tick 2. By HEAD 8b85cab5 it agrees through 30/15. One candidate,
  S118's crossup-heading hunk, was switched off by an env var and the result did not change. But I did
  not confirm that `./kit check` passes the variable through to the test, so S118 is not ruled out. I did
  not bisect the others (S115, S138, MD28, P145, V96 touched the ride since).
- **Only `along`'s tick 1 was Ee-stepped.** `across` agreeing through tick 15 in the replay is the
  evidence for it. Its substeps were not traced.
- **No permanent Ee proof of the contact's placement is committed.** The trace was temporary, as
  S107's was. The replay ratchet holds the result: a moved contact point would part the load point and
  compression lanes at tick 2.
- **Both rides still part**: along at tick 31 and across at 16, and on both at the moment the original's
  engine first drives the rear wheel. The original has `engine.torque` 8.350148 and `wheel_torque` 379.33.
  The port has -0.7397749 and 0. Not looked into here: filed as S183. Until it is fixed, the bed's
  contact and slip lanes past those ticks (`S89_LANES_AGREE_THROUGH`) are still a different run from the
  original's.
- **S124** (what parts along at tick 2) describes a parting HEAD no longer has. I left its checkbox alone,
  since it is not this task.

## Next

S183: S89's rides part where the original's engine first drives the rear wheel.
