# 1 — ai mode drivers (PART)

Task S314. 2026-11-13.

## What I did

- Ported the base AI's two mode drivers as `ai::drive::mode_two` (the race driver, the brain's vt+0x98)
  and `ai::drive::mode_four` (the gate driver, vt+0x9c), plus `ai::drive::path_reach` (the MX brain's
  vt+0x7c, the reach the gate driver uses on a net). The drivers' calls are a `drive::Calls` trait:
  the path net's queries, the gate crossing, and the brain's own slots (reach, route follow, seek,
  avoidance, trick plan, speed step, rider assist).
- Answered the listener half of the task: **no AI brain carries a listener.** In sx01's race image
  (S282c) and in recording2 (free09), every MX brain has an empty list at +0x18 and the listening bit
  (+0x24 bit 1) clear, so `ai::listener_walk` never finds anyone on an AI brain. A test now checks this.
- Named the drivers, vt+0x7c, the speed step and the MX brain vtable in research/oracle.txt
  (`ai_race_drive`, `ai_gate_drive`, `ai_own_path_reach`, `ai_speed_step`, `mx_vehicle_ai_vtable`).
- Filed S314a (port the vt+0xa0 speed step) and S314b (join the drivers into `ai::Racer`'s `Steps`;
  Join still lists "mode two"/"mode four" as missing).

## What the original does

Read with `./kit fn 0x00284de0` (mode 2), `./kit fn 0x00285090` (mode 4) and `./kit fn 0x00283460`
(vt+0x7c), and checked line by line with `./kit dis`.

- **Mode 2**: in a fast-lap race (race +0x60 == 4), a card without its on-run bit (0x100) lets go of the
  controls and answers true. After that it answers false when there is no race, no card
  or no net. A card not taking part (bit 2) lets go and answers true. If the track's line gate has an
  unplaced spot (path or piece 0), it answers false. Then it picks the goal. With the card's kept spot,
  the goal is that spot's point, the card's kept distance and its on-track word. Otherwise it is the
  rider's own position, speed 1000 and word 1. It calls vt+0x78 (the path reach), then the route
  follower with the goal word. Under the bike sweep bit (0x1000) it raises or lowers 0xc0 by on-track.
  Then come the avoidance (bit 1) and the trick plan. It copies the target speed to +0x324, then runs the
  speed step (0x400) and the rider assist (0x800). The assist puts heading.x/z × target speed in
  +0x1c0/+0x1c8 and calls the interface's vt+0x18.
- **Mode 4**: with a net, an unplaced rider is placed. Then it does vt+0x7c → route follow with word 1.
  Under 0x4000_0000 with a next gate, a step that crosses the gate beside it sets the sign-extended
  bit-31 flag (`lui 0x8000` → 0xffff_ffff_8000_0000). While that flag is up, the distance to the gate is
  measured both ways. If it is under the gate lead, or further to the gate than back, the driver looks
  ahead from the net's first path. Otherwise the flag clears (bits 31..63).
  With no net it aims at the gate. The facing is the gate's side vector (side gate, 0x20) or its normal,
  turned to face the rider. The aim point is the lead, or the card's distance to the line when that is
  shorter, less the seek's longest reach, back from the centre. It sets a flat unit heading and the
  target speed. Then the seek reach and hand-off run. **The seek uses the facing turned the other way**
  (along the rider): a second, inverted `c.ole` test, caught by the oracle. A plain gate's hand-off
  takes the normal as it is.
- **vt+0x7c**: if the card's out bit is set, it keeps the position at +0x290. It returns
  base + gain × the flat distance to the spot's point, capped at the brain's maximum.
- Mode 2 is the race driver: all three of sx01's race brains are in mode 2. Mode 4 has not been seen in
  any image.

## How it was proved

All tests are in crates/game-sim/tests/ai/drive.rs:

- `the_race_driver_matches_over_ten_thousand_ticks`: function oracle over 10,000 generated worlds,
  every callee hooked at its real address. It compares the call log (arguments, with pointers mapped to
  what they point at), the answer, and every brain word written. All 11 branches were hit
  ≥ 116 times.
- `the_gate_driver_matches_over_ten_thousand_ticks`: the same for mode 4, 10,000 worlds. All 14
  branches were hit ≥ 117 times.
- `the_own_path_reach_matches_over_ten_thousand_calls`: vt+0x7c over 10,000 calls (capped/under,
  kept/not).
- `the_race_driver_matches_on_sx01s_race_image`: mode 2 on the three real brains in s282c, with their
  real race, card, track and net. Only the callees are scripted.
- `ai_brains_carry_no_listeners`: listener lists and bits on both images.

## Not verified / not 1:1

- **Only the orchestration is proved.** Every callee (path queries, gate crossing, reach, route
  follower, seek reach and hand-off, avoidance, trick plan, speed step, assist) is hooked with scripted
  answers. Their own ports are the earlier tasks' work, and the drivers are not yet joined to them
  (S314b).
- **Mode 4 has never been seen live.** No image has a brain in mode 4, so the gate driver is proved
  only on generated worlds.
- **The fast-lap check reads the card without a null test.** With no card in a fast-lap race, the
  original reads address 0. The port treats this as "no card" (it falls through to the false answer),
  and the generator never makes that case.
- **The race's track and the track's line gate are not null-tested** in the original. The port takes
  them as present (`Race` carries their values). It was not tested with them missing.
- **The point query's answer is ignored.** If `path_spot_point` fails, the original goes on with
  whatever is in its stack buffer. The port uses the returned point. The hooks always succeed.
- **vt+0x7c's float argument is unused** by the original. The port's `path_reach` callout passes it
  through for the log only.
- **The cone constant handed to the seek hand-off** is passed through as the original's literal. The
  hand-off's use of it is not part of this task.
- **The PC build was not read.** Only the PS2 ELF was used.
- **No live capture.** There is no PINE recording of a driver's tick. The proof is the interpreter
  oracle plus the one real-image tick.

## Next

S314b: join `drive::mode_two`/`mode_four` into `ai::Racer`'s `Steps`. S314a: port the speed step.
