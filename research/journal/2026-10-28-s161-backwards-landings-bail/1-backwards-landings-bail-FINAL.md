# 1 — backwards landings bail (FINAL)

Task S161. 2026-10-28.

## What I did

- **Decided to build it.** The brief is conditional ("if the user wants such a landing to bail"), and
  the run is unattended. Added it: off is the default and the original bit for bit (the new clause is
  never asked), and S150 came from the user's complaint that a bad landing "instead of bailing flings
  you around". If the user does not want it, it is one row they never have to touch.
- **The clause** (`air::backwards_landing`, not the original's): on the substep the wheels come down
  after more than `CrossupWreckTime` aloft, with the bike not mid-backflip and faster than `SLOW` (the
  same conditions under which the original measures and judges its crossup), a wreck when the
  flattened forward axis and velocity have a negative dot product, i.e. more than a quarter turn apart.
  Its side is the crossup's sign (Left for a positive cross y, the side the shown crossup's > 0.5
  wrecks to). The rider asks it only when `Sense::backwards_bail` is on (`state::Allowed::backwards`)
  and only after the original's `wreck_test_in` let the landing go, so wherever the original wrecks,
  its wreck and reason stand.
- Took the bike's forward axis from `Ride::axes` rather than `air::Step::heading`: the wreck test's
  Step in `rider.rs` leaves `heading` at zero (only up/stunt/touching/spinning reach the test), which the
  first version read and so never fired.
- **Why not "more than 135 degrees"**: anything more than 90 degrees round already reads 45..90
  degrees in the arcsine, past the 45-degree limit, so between 90 and 135 the original already wrecks
  (S150's 118-degree r50 does). A quarter turn is the clean line the dot product draws, and on its far
  side the switch only changes landings past ~135, the ones the arcsine folds back.
- **Carried like Air Wrecks (P138/P175)**: `Settings::backwards_bail` (`backwards_bail = on` written
  only when on), the Game Modifiers page row under Air Wrecks (world, dimmed with Wrecks off, held where
  Air Wrecks is held — a played tape, an online room — and counted in the page's "N Changed"), Original
  mode (On needs it off; setting On turns it off), the host's All Players reset (turns it off),
  `Sense::backwards_bail` on the snapshot wire, tape lines `backwards_bail FRAME on|off` (Recorder, Head,
  `play_from`), the shell's `--backwards-bail`, the pause's change taken as it closes, a played tape
  starting it off, `Room`/`Handshake` fields (serde default off, refused when they differ).
- opentennis has no such rule (no crossed-up landings in tennis); nothing taken from it.

## What the original does

S150's journal (research/journal/2026-10-26-s150-trick-landing-spin): the landing crossup clause of the
wreck test (`./kit fn 0x002522c0`) judges an arcsine crossup, so a landing θ crossed up reads 180 − θ,
and past ~135 degrees it rides out and the tyres swing it round. This task does not change that path.

## How it was proved

- `game-sim/tests/replay.rs` `backwards_landings_bail_when_the_switch_is_on`: S150's three save-state
  captures replayed with the switch on. r50 (118°): wreck reason 3 at 2822, the original's at 2823 (same
  as off). r60 (148°): wreck reason 3 at 2801, the tick the port and the original touch down. r70
  (169°): reason 3 at 2855, likewise. Run here with `GAME_SIM_FIXTURES=<s2>/context` (the captures live
  in s2's context only); in other slots it skips with a line, like S150's.
- `game-sim/tests/backwards_bail.rs`:
  - the clause against its rule written plainly in f64, over 20,000 generated touchdowns (each landing
    condition sometimes false, forward and velocity anywhere): 2,683 wreck, 17,317 do not, all agree,
    side included; none fell within 1e-4 of a quarter turn.
  - the stand-in bike dropped 12 ft onto a flat floor at 30: going backwards, off rides out and on wrecks
    on the touch tick (29); forwards and 80 degrees aside, on and off both ride out. A new `Rider` has it off.
  - the tape line's round trip, Head and `play_from`.
- `openmx/tests/menu.rs` `the_backwards_bail_row_toggles_and_survives_a_restart` and the settings
  round trip; `openmx-net` handshake test refuses a differing switch.
- Off is the original: every existing replay and S150's own test ride with the default and pass.
- The snapshot wire grew one bool a rider: `platforms.rs`' RIDERS_WIRE_DIGEST/LEN re-pinned (13495 → 13499 bytes, four riders); the per-tick hashes and RIDERS_DIGEST did not move, so the ride itself is unchanged.

## Not verified / not 1:1

- **The whole switch is not the original's**, by design: on, a landing more than a quarter turn round
  wrecks where the original (past ~135°) rides it out. Off is the original.
- **The wreck's reason with the switch on is the port's choice**: Left/Right (2/3) by the crossup's side,
  like the original's crossup clause; there is no original behaviour to match. The reason's side was
  checked only on right-turning throws (S150's three, all reason 3) and the generated sweep.
- **The wreck entry's look** (ragdoll throw for a bike landing backwards) was not filmed or looked at;
  it goes through the same entry as any reason-2/3 wreck.
- **Not ridden in the shell or online**: the menu row, the settings file and the handshake are tested;
  the shell's `--backwards-bail`, the pause-close change and a tape with the line played in the shell
  were not run by hand.
- **The held-row note** reads "The tape or the room holds it for the ride" rather than reusing
  `air_lock`'s message, which names Air Wrecks.
- **The 90–135° band**: with the switch on, a landing there that the original rides out (possible only
  if the shown crossup, a blended pose, lags the measured one) now wrecks too. Not captured.

## Next

Nothing.
