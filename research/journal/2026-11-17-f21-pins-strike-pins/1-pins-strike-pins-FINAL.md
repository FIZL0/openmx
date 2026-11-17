# 1 — pins strike pins (FINAL)

Task F21. 2026-11-17.

## What I did

- A prop's rays now strike the meshes of other bodies in its own collection. Before, `strikes_of` in
  prop_props.rs skipped the whole collection; now it skips only the body itself.
- `strike_props` runs when there are two props in total, not only two collections.
- The step does not copy a sibling body. That would integrate it twice, and the original resolves it in
  place inside one world. `step_named` names a sibling by `others.len() + j`, after the copies of other
  collections' props. prop_world.rs `Lists::force` then hands the collection's own bodies in place
  (`force_among_siblings`; the body's own slot gets a stand-in no entry names).
- Live capture on free11 (copy 5), research/f21:
  - `pins.py` finds the ten-pin collection in a state.
  - The boot has no bike (no human vehicle in the manager list), so the s195 bike teleport could not work.
    `toss.py` instead puts a one-shot fake listener on the props node. Its stub wakes pin 1 the way a
    strike does (activate, then enable) and gives it a velocity.
  - `toss.py 21 1 5.2 6 -2.96 2`, then `strike.py lean 2 --frames 18 --at 209,...,214`.
  - `lists.py` prints the node and lists.
  - States: context/f21/lean-209..214.p2s. RAM images: context/states/f21-lean-*.eeMemory.bin (s5 only).
- tests/prop_props.rs: `a_pin_strikes_a_pin_of_its_own_collection` (each tick from its state) and
  `a_pin_on_a_pin_runs_on` (free-running from 209).
- F19 in plan/TODO.md is the same work; this covers it.

## What the original does

- Each prop's probe ignores only its own mesh collidable. So a sibling is a valid target, the same as a
  prop in another collection. This holds on every multi-body collection in the sx01 and free11 images.
- So within one collection the original runs S242's rules unchanged:
  - The struck pin is woken, filed at the node's end and searched in the same pass.
  - The striker's mirror (kind 1) goes at the head of the struck pin's list when the struck pin comes later
    in node order.
- The capture, tick by tick:
  - 212: pin 1's rays strike pin 0. Pin 0 is filed after pin 1, with pin 1's mirror first.
  - 213: each pin's rays strike the other, in node order.
- Addresses: research/oracle.txt F21.

## How it was proved

- A save-state capture: lean-209 to lean-214 (five ticks), free11's pins collection.
- Each tick from its state, checked bit for bit: the node's order, both pins' lists (kind, from, on_body,
  normal, point, surface, which prop is named) and flags, and both bodies.
- Also run free from 209 to 214: both bodies bit for bit each tick.
- Mutant check: putting back the old whole-collection skip fails both tests. The fix passes both.

## Not verified / not 1:1

- The wake was injected by a stub, not a bike knock. Reason: the free11 boot has no bike. The strike between
  the pins after the wake is the original's own code.
- Only one same-collection pair (free11 pins 0 and 1) was captured. Reason: one capture shows the rule, and
  the ignore lists read the same on every multi-body collection. Other tracks (nat01, nat07, sx) were not
  checked live.
- A collection with joints between its bodies (a sibling both jointed and struck) was not captured. Reason:
  the two pins that met are separate bodies, so the capture does not show it.
- The fake listener's swap-removal from the node list happens in the captured run before 209. Reason: it is
  the injection, not the port's subject. The port has no such listener.
- The states live only in s5's context. Reason: captures are never committed. The tests skip with a line
  where they are missing.

## Next

- Nothing for F21.
