# 1 — trigger contact (FINAL)

Task V104. 2026-11-02.

## What I did

- `creature.rs`:
  - `Trigger` gains `ignore` (IgnoreTrigger bits), `enter_test`/`exit_test`, `touched`/`touched_last`,
    `count`, the contact list `inside` (slots, `None` once an actor has left) and `actions`.
  - Each `Action` keeps TriggerCounterEquals/GreaterThan, TriggerActionDelayTimer,
    TriggerActionUponExit, ActionTest, its timer and pending actor, a run count, and its `Scare`.
    `scares` became the method `scares()`.
- `Creatures::step_among(dt, &[Actor])` runs each tick in this order:
  - each trigger's update: touch bits shift, exit test (`Trigger::keeps`: the actor's tree-root box
    against the trigger box, which is `ray_probe::group_reaches`), UponExit actions, timers;
  - then the strikes (`Trigger::struck_by`), each a `Creatures::contact`.
  - `step(dt, &[Rays])` is still there and wraps it.
- `Actor::Probe` is struck by the new `probe_strikes_box`, the ray class's box strike: the probe tree
  descends through `ray_probe::boxes`, and the leaves go through `segment_touches_box`.
- `main.rs` `creature_rays` now hands the bike's body-ray probe itself (`BikeCol::probe(ride.transform())`)
  instead of the segments put into the world by host f32.
- The V102 per-rider "inside last tick" test is replaced by the original's bits. A chance trigger now skips
  its roll when it was *touched* the tick before, whatever came of that touch. Leaving now means the
  actor's box leaving, which is looser than its segments missing.

## What the original does

Recipes:

- `./kit fn 0x001c6190` (contact), `0x001c7560` (actions), `0x001c7110` (update), `0x0011af30` /
  `0x0011b020` (list add / remove), `0x001c6450` (test combinator eval), `0x002131f0` (test parse);
- `./kit dis 0x0010a9b0` and `0x0010a890` (the box strike and its descent);
- the IgnoreTrigger ORs at 0x1c6ad4..0x1c6b1c.

All of it is written up in research/oracle.txt under "V104".

- **Contact.** It needs the live flag, the trigger enabled, and the EnterTest passing. Then:
  - the touched bit is set;
  - a one-time trigger that has fired stops;
  - under 100 percent, being touched last frame stops it, and so does losing `percent < int(rand·2⁻³¹·100)+1`;
  - then it is fired, count + 1, and the actions run.
- **Actions.** An actor already in the shape's contact list runs none. Each action with a passing
  ActionTest:
  - with no counter keys: a delay starts the timer (whatever UponExit says); else it runs if it is not
    UponExit;
  - with counter keys: it is due when count == Equals, or when Greater != −1 and Greater < count; then
    the timer starts or it runs.
  - After the actions, the shape and the actor go in each other's lists. A list grows through realloc
    and reuses a zeroed slot.
- **Update.**
  - The last-frame bit takes this frame's, and this frame's is cleared.
  - Each listed actor is exit-tested. A miss zeroes its slot. If the ExitTest passes, each UponExit
    action runs with enter = 0 (ScareBirds does nothing on 0), or with a delay starts its timer.
  - **Listing slip:** the delay is copied from the action whose index is the actor's *slot*, not from the
    action itself. The port does the same.
  - Then the timers: `t − dt`; at ≤ 0 the timer is 0 and the action runs (enter 1).
- **IgnoreTrigger** is searched with strstr: "AI" 1, "PLAYER" 2, "ANIMATIONS" 4. nat01's `AIANIMATION`
  is AI only. Nothing reads the byte back (an objdump scan for byte loads at that offset finds only the
  reader's three ORs).
- **Tests.** A name is looked up in the manager's test-kind table. Only the manager's ctor writes that
  table (count 0), and it is empty in the free06, free07 and kiowa states. So a name resolves to null and
  passes. No trigger or action on the disc names a test, or uses a counter, delay or UponExit key.
- **Path cars.**
  - A car's ray collidable runs the collision pass only under DriveOnSoultree.
  - Its "m.col" ray collidable exists only under CollidesWithOthers.
  - The mesh is placed only, never a mover.
  - All 126 `[CarN]` sections on the disc set both keys FALSE. In the s89 free06 state the m.col pointer
    is null on every car.
  - So no retail path car strikes a trigger, and the port has nothing to add for them.

## How it was proved

- **`crates/game-render/tests/trigger_contact.rs`, `the_box_strike_is_the_originals`.** 6,000 made-up
  probe trees (1–11 segments, node boxes the segment bounds scaled 0.8–1.2) in random frames against
  random boxes. Each runs through `ray_tree_box_strike` on the boot ELF and through
  `creature::probe_strikes_box`. All 6,000 agree; 1,232 strike.
- **`the_contact_callback_is_the_originals`.** 400 made-up triggers in scratch RAM (100/50/75/0 percent,
  some one-time, 3–4 actions with random counter/delay/UponExit keys), each with 1–3 actors, run for
  60 ticks:
  - On the original's side, each tick calls `trigger_update` (dt 1/30), then `trigger_contact` for each
    struck actor.
  - The port runs `step_among`. The actors are segment sets about a 10-ft cube, made so that each is
    struck or not and kept or not, in all four combinations.
  - Hooks: the original's exit test answers what `Trigger::keeps` says; `rand` draws from a copy of the
    port's generator; realloc is a bump allocator; the run is a counting stub.
  - After every tick these agree: touch/last/fired bits, count, the contact list slot by slot, every
    action's timer bits and pending actor, the runs in and out, and the generator.
  - Totals: 8,394 firings, 7,549 runs in, 2,404 runs out, 14,944 empty slots seen.
  - Mutations: giving the UponExit delay the action's own value fails at case 4 tick 2 (timer
    0.0667 vs 0.1667). Setting the touched bit after the one-time check fails at case 1 tick 1.
- `tests/creature.rs`:
  - `trigger_keys_read_as_the_original_reads_them` checks the parse;
  - `an_actor_is_in_until_its_box_leaves` checks staying in, a re-strike with no run, and the slot reused;
  - the V44/V75/V102 tests still pass.
- Checked: `./kit check -- -p game-render --test creature --test trigger_box --test trigger_contact`:
  13 passed. `./kit check -- -p openmx --bin openmx -- <no match>` builds the shell.

## Not verified / not 1:1

- **The order within a tick.** The port runs every trigger's update and then the strikes. In the
  original, the trigger's update and the bike's collision pass are in separate parts of the frame, and I
  did not find which comes first. The touch bits come out the same either way. What changes is whether
  an actor that has just come in is exit-tested on the same frame.
- **Strike order.** With several riders and several chance triggers, the port goes trigger by trigger,
  then rider by rider. The collision pass's order of pairs is not established, so the draws could be
  taken in another order (riders are players only here, so this needs 2p).
- **Exit test for `Actor::Rays`** (points and test segments) is the original's tree-less ray rule, on the
  first segment. The shell now passes the real probe, whose tree root is used.
- **A sphere trigger against a probe** puts the probe's segments into the world by host f32 and uses
  the old distance test. No trigger with a scare is a sphere.
- **The listing slip past the end.** When a leaving actor's slot is beyond the action list, the original
  reads past the list. The port uses the action's own delay. The oracle keeps slots within the list.
- **Tests with operators** (`A&B`) would call through a null child in the original. The port passes them
  as it passes every test. None exist on the disc.
- **Not run in PCSX2:** no capture of a contact. The proofs are function oracles on the ELF with made-up
  objects. The trigger's +0x29 enable bit and the live flag are set by hand.
- **AI riders.** IgnoreTrigger is never read, so an AI bike would scare birds in the original *if* AI
  bikes run the body-ray query. The port has no AI riders, and I did not check whether they do.
- `Scare.delay` (ScareDelay) is still read and unused. A nonzero one takes another path in the scare
  (milliseconds). nat02's are 0; filed as V119.
