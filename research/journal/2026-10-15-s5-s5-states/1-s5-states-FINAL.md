# 1 — s5 states (FINAL)

Task S5. 2026-10-15.

## What I did

- Ported the main actor's state machine to `crates/game-sim/src/state.rs`: `Ride`, the state the machine
  owns, and `Ride::step`, the machine itself — plus `enter`, `request_reset`, `respawn`, the pad chord
  (`chord_allowed` / `take_chord` / `frame_start`) and `phase()`, the three-way view the rest of the actor
  reads. One new module and one `pub mod` line in `lib.rs`; nothing else in the crate moved.
- Wrote five proofs in `crates/game-sim/tests/state.rs`: the machine against the original frame by frame,
  the machine against the original over every combination of what it is told, both halves of the wreck
  entry, the put-back, and the frame counts of the two clocks port-side.
- Registered 17 names for this area in `research/oracle.txt` with their field offsets.
- Added the module to `PLAN.md`'s code map.

The shape worth knowing: the two things the machine *asks* — "does anything wreck this rider now?" and
"is a wreck page already up?" — are arguments, not ported code (`test: impl FnOnce(Allowed) -> Option<Wreck>`
and `pages: impl FnOnce() -> bool`). The wreck test itself is a different subsystem's (S10's landing test
feeds it), but the two gates *in front of* the call — the global wrecks-enabled flag and "already down, so a
landing cannot wreck you again" — belong to this machine, so they are ported here and handed to the closure
as `Allowed { wrecks, landing }`. That way a proof can pass in a stand-in that decides from RAM and the
gates are genuinely under test, rather than being assumed by a test that already knows the answer.

## What the original does

The machine is the last thing the vehicle update runs each frame, called with that frame's `dt`
(`./kit fn 0x0027bc90`). Reading it needs the frame around it (`./kit fn 0x002520d0`), because the single
most consequential fact about this state machine is not in the machine: **the base vehicle update runs the
physics step only while the reason field is 0.** A down rider's bike is not simulated at all — no integration,
no collision, no air state. That is why this is the axis everything else hangs off, and why the port's
`frame_start` → physics-only-while-riding → `step` order is documented in the module rather than left to the
caller to guess.

Three axes, not one. Where the bike is (air/ground) is S10's, what the rider is doing (a trick) is S11's,
and *whether it is riding at all* is this one. They are independent: you can be airborne mid-trick and
wrecked, and the wreck is what stops the other two mattering.

The machine's own flow, each frame:

1. If this rider is driven — an AI route is following, or the player record at +0x20 says control source 1,
   a local human — then ask the wreck test (slot +0xfc's gate), and on a non-zero answer run the wreck entry.
2. If the reason is now non-zero, advance the clock at +0x794 by `dt`. Which clock depends on the latch at
   +0x74c and the settle gate at +0xd3c: with no settle gate, or with the latch already set, the clock just
   adds. Otherwise it is in the settle window first — a comparison of the clock against 0.5 s (2.5 s for
   reason 13, the cannonball) that **tests before adding**, and on the frame it fails sets the latch and
   puts the clock back to 0 without adding to it.
3. Once latched, compare the clock against MaxWreckTime at +0x798 (CannonballWreckTime at +0x79c for reason
   13) — this one **tests after adding**. Past it, ask the front end whether a wreck page is already up (two
   field reads through the page handle, `+0x54` and `+0x48` of `group[which]`, where the groups are an array
   of pointers at handle+0x40) and if not, request the reset: kind 1, with the wreck page for every reason
   except 14.
4. If there is no driver at all and the rider was not already down, enter reason 14 (no control) and preload
   the clock to exactly MaxWreckTime with the latch set — so the reset lands on the first frame somebody
   drives again, and reason 14 is the one reason that resets with no page.
5. Whenever the reason is non-zero, the race record's riding bit (0x80000 of the long at +0xf9c) is cleared.

The entry (`./kit fn 0x00252ca0` then `./kit fn 0x0027b810`) is two halves: the base writes the reason, zeroes
the clock and clears the latch, then places the rider (a running trick stopped, three `rand()` draws for a
cannonball's drop, the wheel loads zeroed); the player's override ends the jump record first, runs the base,
tells the camera/rumble/announcer/front end, and then sets the shown flag at +0x14b4 to `reason != 0` and
counts the entry at +0x12f8.

The put-back (`./kit fn 0x0027bb20` over `./kit fn 0x00252ee0`) clears the reason, clears the attitude vector
at +0x550, doubles a cannonball's damping at +0x6a8 back, puts both action ids at +0xa78/+0xa7c to the
grounded idle (0x1c), sets fresh at +0x750, empties the trick queue, plays a sound, puts the race record's
riding bit back and clears the shown flag. It deliberately **leaves the clock and the latch alone** — they are
reset by the next entry, not by the put-back, which is why the port's `respawn` does not touch them either.

The reset request (`./kit fn 0x0027b9f0`) is the one branch point: with no player record it returns before
doing anything (an AI rider is never put back by this path), and with a screen and a page it raises the page
and waits; otherwise it respawns on the spot, closing any page first.

The pad chord (`./kit fn 0x0027bef0`) is the player's manual way out: gated on no AI route, the one-frame
latch at +0x14d0 being clear, the mode, and the reason not being 13, it asks for kind 1 with no page. The
three gates that are the machine's (route, latch, cannonball) are ported; the pad bindings and the mode
check are not, see below.

## How it was proved

All five in `crates/game-sim/tests/state.rs`, through the function oracle (no emulator):

1. **the machine, frame by frame** — 10,000 generated frames against `0x0027bc90` itself, the port stepped
   beside it with the same `dt` and the same answers, comparing all nine fields of the state plus the
   riding bit and the reset request every frame. Bit-exact. Coverage the run asserts on: 2311 entries, 286
   re-wrecks while already down, 268 put back, 394 frames where a page was already up, 757 arming frames.
2. **the machine, every combination** — 1440 "tellings" (driver × owner × screen × wrecks-enabled × settle
   gate × page answers × reason) and 624 clock states (clock value × latch × reason × limit), each one
   frame, bit-exact.
3. **the wreck entry, both halves** — the base entry for all 15 reason codes with and without a running
   trick, truncated at the first VU0 transform after its three state writes (`0x001b3060`, the hook points
   the return at the oracle's sentinel); then the real player entry for all 15 reasons × owner × screen
   against `Ride::enter`, comparing reason, clock, latch, shown flag and both counters.
4. **the put-back** — the real `0x0027bb20` over four reasons × two stored kinds × owner, against
   `Ride::respawn`: reason cleared, fresh set, shown cleared, clock and latch **unchanged**, the riding bit
   back on, and the kind handed to the placement equal to what `respawn()` returns. The assertions also pin
   the parts this module does not own but must not break: the grounded idle in both action ids, the trick
   forgotten, and a cannonball's damping doubled back.
5. **the clocks, in frames** — port-only arithmetic, because this is what the module's docs claim: at
   `dt = 1/60`, the entry frame is the clock's first, 31 frames add, and the 32nd arms the latch and zeroes
   the clock without adding; then 181 more to the 3 s limit, ending in a page. Then a bike (no settle gate)
   held down for 600 frames with no reset, out only through the chord, and the chord latch lasting exactly
   one frame. Then the no-driver path: down and armed at the limit in one frame with no reset on it, and
   the reset on the first frame somebody drives again.

`./kit check`: all tests pass. Proofs 1 and 2 passed on the first run, so to check they have teeth I ran four
mutations of `state.rs` — the settle window 0.6 instead of 0.5, the already-wrecked gate dropped, a page for
every reason, and the arming frame advancing the clock as well as zeroing it. All four failed both proofs,
and the original passed again.

## Not verified / not 1:1

- **The settle gate at +0xd3c is never written anywhere in the image.** I found no writer, which means for a
  bike it reads whatever the constructor left — and if that is 0, the settle window never ends, the latch is
  never set, and *the entire automatic reset is unreachable*: a wrecked rider waits for the pad chord. The
  port honours both branches (`Sense::settles`) and proof 1 exercises both, so the machine is right either
  way, but **which branch the retail game actually takes is unproved at runtime.** Cheap experiment for
  whoever needs it: wreck a rider and read +0xd3c, +0x74c and +0x748 live over PINE, or from a save state.
  This is the one thing in this task I would not ship a release on.
- `MaxWreckTime` (+0x798) and `CannonballWreckTime` (+0x79c) are read as given; they come from the bike's
  `.phy` file and there is no parser for it yet, so the retail values are unknown. Proof 5's 3 s is the
  proof's own number, not the game's.
- The control source at +0x34 is ported as the one case the machine distinguishes (1 = local human). The
  other values (0 and 2) and the remote/packed-state path they lead to are not ported — a networked or
  replayed rider is out of scope here.
- The race record at +0xf9c and its bit 0x80000: the port models it as a `riding_bit: Option<bool>` output
  and the proofs compare it against the original's write, so the *behaviour* is exact, but what that record
  is and what else reads that bit is not established.
- The presentation half of the entry is not ported: the camera cut, the rumble, the announcer, the front-end
  notice, and the three `rand()` draws a cannonball makes for its drop. The draws matter — they advance the
  shared RNG — and they are hooked out in proof 3 rather than modelled, so **a port that drives the RNG will
  diverge on a cannonball until they are ported.** That belongs with whoever ports the drop itself.
- The per-jump accumulators the entry resets, and the wheel loads it zeroes, are other modules' state.
- The pad chord's own gates are only two-thirds ported: the route, the latch and the cannonball are here;
  the two pad bindings (0x4c, 0x4d), the owner's pad index and the mode check are not, because the pad
  binding layer is the input module's.
- The split-screen `which` argument is not modelled. The port asks `pages()` once for this rider's screen;
  the original indexes a group array by screen. The proof lays the answers out per screen so reading the
  wrong one fails, but the port has no screen index of its own to get wrong.
- In proofs 1 and 2 the entry and the put-back stand in for themselves (the port's `enter`/`respawn` are
  called where the original would call its own). Proofs 3 and 4 are what justify that, against the real
  functions; the composition of all three in one oracle call is not proved end to end.
- Nothing was run on the emulator for this task, so no live capture: everything here is the oracle over the
  original's own code.

## Next

Nothing outstanding for S5 itself. The live read of +0xd3c (first bullet above) is the follow-up that would
turn the biggest "unproved" in this entry into a fact, and it is a few minutes of PINE work during a wreck.
