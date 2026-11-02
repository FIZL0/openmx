# 1 — 2p blend residue (FINAL)

Task V109. 2026-11-02.

## What I did

- **Took the replay apart a layer at a time** (scratch, on C11's `2p-eased` capture). V90's
  `two_p_poses::run_with` became `run_seeded(cap, posed, Seeds)`, which can seed the eased axes and
  reverse clock (C11's `seed`), the clip time (the motion object's last sample time, read out of
  `m_clock` word 0xc), and the snapshot (below). Results, joints within 1e-4 (P1 33 / 34 / 35):
  - eased: 3/17, 10/24, 1550/2200 (C11's numbers).
  - Also seeding the ramp from the row before: no change. Setting the pose to the original's joints of
    1 or 2 rows back before the snapshot: no change, or worse. Re-taking the snapshot every tick: no
    change.
  - Also seeding the clip time: P1 barely moves, while P2 goes riding 6320→7409, air 388→806, 33 13→38,
    34 20→30.
  - Per tick, the residue appears only while the ramp is non-zero, and it scales with the ramp. At ramp
    0 the joints match to ~3e-7. So the error sits in what the fade blends from (the snapshot), not in
    the clips. Inverting the slerp tick by tick shows the original's snapshot is constant through a fade
    but 0.05–0.6 rad away from the original's own blend buffer of the row before.
- **Captured the snapshot.** `research/v109/make_spec.py` → `research/v109/snap.toml`: C11's machine
  words plus, per rider, each joint record's transform node (+0x34) with its local matrix's four rows,
  and the motion object's snapshot buffers (+0x248 quaternions, +0x24c translations). Played on kit copy
  2 with `SPEC=research/v109/snap.toml research/c10/play_capture.sh … 139950 471` (NominalScalar 0.25,
  `[Pad2] DualShock2`). The tape was a temporary copy, so `2p.p2m2` is untouched (md5 a6e23e91…).
  - The result is `context/capture/v109/snap.csv`: 471 rows, frames 139950..140420, no gaps, 844 s.
    Copied to s1, s3, s5 and merge (not s4 or the main checkout). The copy was stopped afterwards.
- **What the capture shows** (see the oracle.txt note under the snapshot buffers):
  - There are 8 restarts in the window. P1: 140354→34, 140378→33, 140397→30. P2: 139972→35, 140036→30,
    140243→35, 140364→30, 140365→40 (wreck).
  - At each restart, the new +0x248 equals `mat4_to_quat` of the joints' nodes **as the row before left
    them**, to 0.0000 rad. Against the joint records' blended quaternions (+0x50) of that row it is
    0.15–0.85 rad off.
  - Between restarts, +0x248 does not change.
  - While riding, the nodes differ from the blend buffer by ~0.03–0.3 rad, and by much more in the
    wreck: the riding ragdoll (S109) writes its bones back into the nodes after the blend.
  - +0x24c is not rewritten at a riding restart: it holds the same translations at all seven. At the
    wreck entry it is rewritten, equal to the nodes' row 3. The riding fades never read it, since
    seeding the rotations alone closes them.
- **Proved it in the port's replay.** `Seeds::nodes` (`two_p_poses::node_trees`) does the following on
  a restart or snapshot tick: it sets `Set::under` to the original's node tree of the row before (rows
  through the port's `mat4_to_quat`), and posing then goes ahead with no rewind of its own.
  - With eased + clip time + nodes, P1's 33 goes 3/17 → 17/17.
  - P2's air entries (139972, 140243) and air exit (140036) drop from 0.71–0.75 rad to under 1e-6.
- Test: `the_2p_fades_start_from_the_ragdolls_nodes` (`crates/game-sim/tests/rider_anim.rs`) pins
  both halves:
  - In the capture, every one of the 3 + 5 restarts snapshots the nodes of the row before.
  - In the replay, every fading tick inside the capture's window whose ramp the port has bit for bit
    (14 for P1, 50 for P2) poses within 1e-4 off the nodes. With the port's own snapshot, the worst of
    those ticks is >0.1 rad out (0.74 for P2).
  - It skips without either capture. V90's and C11's pins still pass unchanged.

## How it was proved

- `cargo test -p game-sim --test rider_anim`: 16 passed, the new test included.
- `./kit audit`: the only leak is the existing `plan/DONE.md:519`.
- Python checks of the snapshot and node columns (normalised quaternion dot products) were run on the
  capture. The test re-does the rotation half in Rust.

## Not verified / not 1:1

- P1's 34 entry (140354) and its return to riding (140397) stay at ~4e-2. There the port's ramp is two
  steps ahead of the original's: 0.7997 against 0.9332, so the port restarted 2 ticks early. That
  timing is a separate gap, which seeding the snapshot cannot close. Filed.
- P1's air (1550/2200) has no fade inside the capture's 471 frames, so the air residue outside the
  window is not proved to be the same layer. That is likely, because the air arm is a fade too.
- That the riding ragdoll is what writes the nodes is inferred, not watched: the nodes part from the
  blend buffer by ragdoll-sized amounts while riding and by much more in the wreck. No write
  breakpoint was set.
- Why +0x24c is skipped at a riding restart (a flag? a per-clip translation channel?) was not traced.
- The port is not changed. Its snapshot is `skeleton.nodes(blend buffer)`. Matching the original
  needs the riding ragdoll's bones as the snapshot source, while the shell's drawing of the riding
  ragdoll stays off by default (the user's rule). Filed as a follow-up.
- The two captures are separate playbacks of the same tape, and they part in the low bits. The pins
  hold at 1e-4, not bit for bit.

## Next

- The follow-ups: the snapshot source in the port, and P1's early reverse restart.
