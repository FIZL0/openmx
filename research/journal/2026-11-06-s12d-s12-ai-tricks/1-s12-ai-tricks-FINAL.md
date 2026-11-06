# 1 — s12 ai tricks (FINAL)

Task S12d. 2026-11-06.

## What I did

- **Found the caller.** `stunt_ai_choice` has one caller, a vtable thunk (`./kit dis 0x00281bb0 0x00281bc0
  -p boot`: `j 0x280df0; addiu a0,a0,-0x10d0`). The rider's AI interface is a sub-object at rider +0x10d0
  whose vptr sits at its +8 (old g++ layout); its vtable is `ai_stunt_vtable` 0x004550e4, slot +0x24 the
  choice, +0x14 "do this trick" (`ai_ask_for`, 0x00280f00, vtable-only: read with `./kit dis`). Grepping
  boot.c for `+ 0x40) + 8) + 0x24))` finds the AI brain's trick decision, `./kit fn 0x00283660`
  (`ai_trick_plan`). The brain is the 0x400-byte object `./kit fn 0x00384cc0` allocates and stores in rider
  +0x10d0 — so `ai_gesture_gate`'s +0x10d0 test is "this rider is AI-driven".
- **Ported** in `crates/game-sim/src/ai.rs`: `plan_trick` (the decision), `ask_for` (the "do this trick"
  call), `gate_holds` / `gesture_gate` (the AI's request gate, falling through to `Tricks::request`), the
  brain flag bits it reads in `ai::flag` (`TRICKS`, `TRICK_READY`, `NO_TRICKS`, `BALLISTIC`, `TRICKED`) and
  the `TrickPlan` / `Flight` / `Jump` inputs. Nothing in `trick.rs` changed: the chooser stays
  `Set::ai_choice`.
- Named `ai_ask_for`, `ai_stunt_vtable`, `ai_trick_plan` in `research/oracle.txt`, and noted what
  `ai_gesture_gate`'s +0x10d0 is.
- opentennis has no trick AI; nothing to take.

## What the original does

`./kit dis 0x00283660 0x002838a0 -p boot`, once a frame with dt, on the brain (fields at brain offsets):

1. It **rolls** when the interface (+0x40) is attached, the flag word (+0x30, u64) has 8 (ready) and 2
   (tricks on) and neither 0x10 nor 0x20 (on the ground), and either the route node (+0x224) is a jump
   (node +4 bit 0x20) or flag 0x2000 (ballistic) is on. The AI set-up (`./kit fn 0x00384cc0`) turns 2 and
   0x2000 on, cool-down +0x158 = 10, chance +0x160 = 10.
   - `since (+0x15c) < cooldown` → clear 8.
   - Estimate +0x19c: not ballistic → `node+0x48 − air (+0x200)`. Ballistic: vertical speed (+0x54) > 0 →
     `0.0625·v`; else `0.03125·v + sqrt(0.0625·(height +0xc4 − ground +0x204))`. The ground is the one the
     rider-to-brain copy writes (`./kit dis 0x0027fd40 0x0027fd78 -p boot`), which also sets 0x20 from the
     rider's ground word.
   - `100·(2⁻³¹·rand())` over the chance → clear 8. Else the choice with `estimate − 0.5` into +0x1a0; if it
     found one, "do this trick" with it and set 0x80000.
2. Otherwise **since += dt**, and on the ground set 8.
3. Then off the ground **air += dt**; on it air = 0, and if 0x80000 is set since = 0 and 0x80000 clears.

"Do this trick" (0x00280f00): slot 0..23 → requested = slot, restart = 1, armed = 0, return 1; else 0.
The gate (0x00280f30): brain non-null and (restart or in progress) → 1, else `stunt_request`.

A roll that picks a trick leaves 8 set, so every following air frame rolls again (10 % a frame on the set-up
values) and can ask for another trick, which restarts the one running. Kept as the original's.

**SLUS-21104** (`./kit fn 0x004ac020 -p atv`) is the same function 0x10 fields higher, with one change: in
ballistic mode a jump node with a positive air time still wins over the ballistic estimate. MX vs. ATV's
change; the port follows MX Unleashed. The PC exe is not on this slot (no `context/mxvatv-pc/`), so it was
not read.

## How it was proved

Function oracle, `crates/game-sim/tests/ai.rs`, all against the boot ELF in `ps2_oracle::Ee`:

- `the_trick_decision_matches_over_ten_thousand_frames` — 10,000 generated frames through the original
  `ai_trick_plan` with the rider's **real** interface vtable, its thunks, the real `stunt_ai_choice` and
  `ai_ask_for`, and the real `rand()` (only `strcmp` is stood in for, as in tests/trick.rs). Flags drawn over
  every bit read plus two unread ones, the interface absent 1 in 16, node none / not a jump / a jump, signed
  zeros, dt 1/30, 1/60 and 0, chances near 0, 10, 50, 100. Compared bit for bit: the flag word, since,
  estimate, slot, air, the rider's requested/restart/armed, and the generator state. The run counts each
  branch and fails if one is missed; the committed seed reached: no roll 7082, cool-down 1418, trick asked
  160, failed roll 917, nothing fits 423, route estimate 508, rising 456, falling 536, tricked landings 1834,
  no rider 608.
- `ask_for_matches_every_slot` — slots −40..59 × restart × armed against 0x00280f00.
- `the_gesture_gate_matches_every_state` — brain null/non-null × restart {0, 1, 0x100} × in-progress {0, 1,
  0x80000000} against 0x00280f30 with `stunt_request` hooked: whether it holds or hands over. The hand-over
  itself is `Tricks::request`, proved in tests/trick.rs.

Each test was made to panic once at its end to show it ran rather than skipped.

## Not verified / not 1:1

- **No live capture of an AI rider tricking.** The decision is proved as a function; that it is called once
  a frame, where in the frame, and with which dt is S12e's (AI riders in the Sim, the free09 4-racer
  capture). Nothing calls `plan_trick` or `gesture_gate` in the port yet.
- **What feeds the brain fields is not ported:** the vertical speed (+0x54), height (+0xc4), ground (+0x204)
  and the on-ground bit come from the rider-to-brain copy (0x0027fb30), the route node (+0x224) from the route
  follower, and the cool-down/chance from the AI set-up and its character file. Taken here as inputs; S12e.
- **Flag 0x10** (`NO_TRICKS`) holds the roll off, but nothing was traced that sets it. Its name is a guess at
  its role.
- `strcmp` in the chooser is the test's stand-in, as in tests/trick.rs (the interpreter lacks two quadword
  compares).
- The PC exe was not read (not on this slot); SLUS-21104 was.

## Next

S12e: wire `plan_trick` and `gesture_gate` into the AI rider's tick, with the rider-to-brain copy.
