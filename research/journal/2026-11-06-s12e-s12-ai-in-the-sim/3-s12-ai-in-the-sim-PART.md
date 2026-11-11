# 3 — s12 ai in the sim (PART)

Task S12e. 2026-11-11.

## What I did

- **Checked what S12e stands on, now that S281, S282 and S283 have landed.** They did not leave
  enough to make the proof the task asks for: a free09 4-racer replay of `rand_state` and the AI
  bikes' bodies. On free09 the opponents are steered by the target seeker (`ai::seek::seek`, S282a),
  and three things it needs are still not in the port:
  - **The seeker's world.** No port code builds an `ai::seek::World` (target sets, targets, jumps)
    from the disc. `grep` finds no constructor outside `ai.rs` and its tests. S305 reads one out of
    recording2's RAM image, but only as a check. A Sim ride needs the map load to build it.
    I filed this as **S313**.
  - **The pick list** (S306). `Set::picks: None` picks nothing, so a seeded AI rider would ride no
    target, and the original's set choice and its draws would be missing.
  - **The tick's unported callouts** (S312). The hit reaction (interface vt+0x3c) draws the swerve bar
    with `rand()`. The approach set-up (vt+0x28) runs under flag 0x200000. The stuck rescue moves the
    bike. Any one of them, left out, puts the generator or the bodies off the original within a few
    hundred ticks of a 4-racer ride, so the replay cannot match.
- **Did not write a partial wiring.** A `Steps` impl and an AI `Player` lane in `Sim::step_all` with
  stubbed callouts and an empty world would be unproved. It would also need rewriting once S312, S313
  and S306 land, and it would touch `lib.rs`/`tick.rs`, which other tasks are editing now. The pieces
  it would join are each already proved on their own (feed, seek, follow, controls, avoid, plan_trick,
  update, hand_off, assist). What is left to prove is the join, and only the replay proves it.
- Blocked `--after S312,S313,S306`. S305 is not on the list: it is a check of the same data S313
  loads, and S313's proof covers it.
- opentennis has no rider AI to take from (as in part 2).

## What the original does

Nothing new studied this session. The order to wire is in S281's journal: stuck clock, ground-bit
carry, feed (vt+8), route (`ai_tick_flag`: seeker on free09, path follower on race tracks), controls
(vt+0x90), approach (vt+0x28 under 0x200000), extra (vt+0xa8 under 0x4000), climb, hand-off (vt+0xc),
listeners.

## How it was proved

Nothing new was proved. No code changed.

## Not verified / not 1:1

- **The whole task is still undone.** None of these exist: AI riders spawned into the Sim, stepped in
  actor-list order, the `rand()` interleave, `plan_trick`/`gesture_gate` wired per rider, or the free09
  4-racer replay. Reason: S312 (callouts, including a `rand()` draw), S313 (seeker world from the disc)
  and S306 (pick list) are not ported, and without them the replay cannot match.
- **That the three are the last missing pieces is a reading, not a proof.** Other gaps may show up
  once the replay runs: the race card's running bit (S281, not captured), the 2-row ground-bit lag
  S281 left open, and whether free09's AI bikes ride Race125A's `.phy`.

## Next

Once S312, S313 and S306 land: give `Sim` an AI lane (one `Player` plus `ai::Brain`, seeker,
`Character`, per roster entry from `ai::spawn_roster`, in actor-list order). Implement `ai::Steps` on
it from the ported pieces, call it before each AI bike's step, and record the free09 4-racer capture
(`rand_state` plus each AI body) for `tests/ai.rs`.
