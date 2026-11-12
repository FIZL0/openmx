# 1 — untamed atv (FINAL)

Task MD81b. 2026-11-12.

## What I did

- Compared MX vs. ATV Untamed's (SLUS-21701) quad step with MX vs. ATV Unleashed's (SLUS-21104) statically,
  as MD81a did for the bike. I paired twins by layout order on the folded ELF, disassembled each pair at the
  larger Ghidra size with `--limit 100000`, sdiffed them, and walked the `jal` callees three deep from the
  quad's ground step, ground head, air state, update and wheel contacts. The verdict table is in
  research/untamed-machines.md ("SLUS-21701's ATV step against SLUS-21104's (MD81b)"). The addresses are in
  research/oracle.txt `# MD81b`.
- Ported the two behaviour changes on the quad's path that the bike's MD93 work had not already ported. Both
  are off by default:
  - `handling::untamed::stuck`: Untamed's hazard clock, with its `grounded` word. The quad switches to it
    with `quad::Tail::untamed`, which gives the new `quad::Call::UntamedHazardSurface`.
  - `atv_moto::Scene::update_untamed` and `atv_moto::untamed_orthonormal`: Untamed's scene-node world
    update.
- Split the task. The capture is MD143. The pack, the Steps routing and the replay are MD144, after MD143.
  The bike's and the cars' callers are MD145.

## What the original does

- **The quad step is SLUS-21104's on moved fields.** The ground step is the same code. The wheels, engine,
  suspension, tire forces, clutch, shift, drag, stand-upright, roll/pitch and contacts differ only in
  scheduling, field offsets and vtable slots (8 lower). The only change in airborne control is the trick
  bounds (41 → 49, 46 → 54), and the port does not read them. The quad's own air state renames a register
  and grows its frame by 16 B. It makes no vtable call, so it has none of the bike's out-of-world rework.
- **The quad shares the bike's rework.** Its ground step calls the bike's preload, backflip and spin in the
  air, which Untamed rewrote (MD93, MD93a, MD93b ported them). Its update calls Untamed's bike vehicle update
  (MD93c).
- **The hazard clock has a third word.** The update now calls the clock directly, not through the vtable.
  Beside `on` (hazard and any wheel's `+0xd0`) it hands `grounded`, which is any wheel's `+0xd0` alone. When
  `grounded` is clear, the clock skips the free reset: the vehicle is not free, it is stuck unless its run's
  flag says not, and the clock runs by the four wheels' longest surface time. That time is −1 for an
  immediate put-back, ≤ 0 to zero the time, and > 0 to count up and put back once past it. When `grounded`
  is set, the clock is SLUS-21104's. The bike's and the car's updates call the same clock (MD145).
- **The node world update re-orthonormalises.** After the topmost stale node's frame is multiplied by its
  parent's, Untamed rebuilds it. The first row becomes y × z, the second becomes z × that, both are scaled
  by `rsqrt` of their length², and the first is made again as their cross. The nodes below it are
  multiplied as before. This is the scene graph, so it applies to every machine.
- **The update's distance stat.** `if d < 0 { d = 0 } dist += d·dt` became `if !(d <= 0) { dist += d·dt }`.
  The two differ only on a −0 distance, which the stat never holds. Not ported.

## How it was proved

All three are function oracles. Each runs SLUS-21701's own code in the Ee interpreter on
`context/untamed/ram/nat_or.bin` (an Untamed Free Ride RAM image), with the inputs laid out at Untamed's
offsets. The tests are in `crates/game-sim/tests/untamed_atv.rs`:
- `untamed_hazard_clock_is_the_originals`: 4000 generated states. It checks free, stuck, the time's bits and
  the put-back count. It also asserts that SLUS-21104's `tail::stuck` gives the same answer whenever
  `grounded` is set. A probe run showed the tally: grounded 2001; not grounded, as SLUS-21104 600; kept
  stuck 1070; put back 329.
- `untamed_orthonormaliser_is_the_originals`: 3000 frames. Each is a quaternion frame, sometimes nudged off
  orthonormal, and now and then all zeros. Every word must be bit-equal.
- `untamed_node_world_update_is_the_originals`: 2000 three-node chains with random current flags, updated
  from a random node. Every world word and flag byte must be bit-equal. A probe run showed the tally:
  current 999, as SLUS-21104 593, made orthonormal 408.

The rest of the comparison is a static diff, not a run.

## Not verified / not 1:1

- **No Untamed ATV ride was captured or replayed.** The brief's test, a captured SLUS-21701 ride replaying
  bit-exact, is not done. No Untamed ATV save state exists (copy 9's three are a bike Free Ride). Getting
  one needs a tape-driven menu run, a C28-style recorder on Untamed's offsets, and an Untamed sim-frame
  harness. That is MD143, then MD144. I did not start the PCSX2 copy.
- **No pack.** The openmx-mods machine pack (tools/import-atv) is another repo's work, and it would ride the
  SLUS-21104 step until quad::Steps routes the Untamed calls. Both are MD144.
- **Nothing calls the two ports on a ride.** `Tail::untamed` defaults to false and `update_untamed` has no
  caller, so every existing replay and hash is unchanged. They are proved only against the original
  functions, not on a ride.
- **What `grounded` means is still a guess.** It is any wheel's `+0xd0`, the port's `wheel_sunk`, whose name
  was guessed from use (MD64a). The port hands it through as it comes.
- **The static diff did not cover everything.** These differ but are unclassified: plant axis-angle (likely
  a size split), quat-to-euler (a new `jal` past its end), collide-against search, pair accept, eco part,
  event post, two unnamed update callees, and the name bit. These were never reached: the quad's `.phy` tire
  reader, put-back, animate, node pose, net unpack, and the wreck path (wreck test/entry on the quad). The
  list is in research/untamed-machines.md.
- **The static diff is pattern work.** Each diff hides register renames and scheduling. A "same code"
  verdict means the sdiff showed only those, not that both were run.
- MD81a's diffs ran under `./kit dis`'s 400-line default, which may have cut long functions short. I did
  not re-check MD81a's verdicts.
- The distance-stat change is not ported (above).
- **Two C28 port replays are red, not from this change.** I copied kept-context/md37a into this slot for the
  harness. With its csvs present, the first gate failed `race_stoppie_short_replays_by_the_port` (frame 109)
  and `race_wheelie_launch_replays_by_the_port` (frame 88) in the quad record's sweep words. Both fail the same
  way on the task base without my change, and with main's MD119 sweep fix cherry-picked on top, so I dropped
  the cherry-pick. The 13 other port replays and all the originals'-frame twins passed with my change in
  place. That is the evidence that the hashes and replays are unchanged with `Tail::untamed` off. I filed
  MD146 and moved capture/md37a back out of the slot, so the final gate skipped the C28 replays as every slot
  without them does.

## Next

MD143: capture an Untamed ATV state and rides. Then MD144: route, pack, replay. MD145 is the bike's and the
cars' hazard clock and node update. MD146 is the red C28 port replays.
