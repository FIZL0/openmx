# 1 — biplane turn modifier (FINAL)

Task P108. 2026-10-22.

## What I did

- **Looked for a sim bug in the slow turn first, and found none.** The port's turn is the original's:
  - The biplane acceptance run (`replay.rs`, `the_biplane_acceptance_run`) agrees to the bit through tick 691. That run includes the human's banked pulls at 77–115 ft/s, at heading rates up to about 107°/s (`turns.py` over the acceptance csv).
  - Every listener on the turn's path is oracle-proven over generated cases in `tests/biplane.rs`: the controls' roll fade, the autopilot's PI, aero, the integrator and the measure.
- **What holds the turn.** I measured it with a scratch probe, since deleted, that flies `Flight` on the disc's file with a hand holding the bank.
  - **Original scheme.** The elevator fades to `MinElevator` 0 between 30° and 60° of roll. On its side the stick does nothing, so the plane turns at 30–46°/s.
  - **New scheme** (vehicle.rs every-bank + round stick, the default since 700e411). The elevator reaches its stop, `MaxElevator` 0.5236. There `CMalpha·α = −CMde·δe` trims α ≈ 0.22, which is the top of the lift curve (`CL = 5.55α/(1+(3α)^6)`, peak ≈ 1.18 at α ≈ 0.255).
  - So the turn is **lift-limited**: at the stock wing loading the stick cannot pull harder. Measured 68–89°/s on average over 3 s, from 60–220 ft/s.
  - It is not the rate limit: `MaxPitchRate` × 2.5 is 3.1 rad/s, and the pitch rate gets there.
- **The knob, `Knob::PlaneTurn`** ("Biplane turn speed", word `plane_turn`, per seat, on the menu after Biplane top speed). Its effect, as a factor k on the file:
  - `CLScale`, `CMq` × k;
  - `CMalpha`, `CMde`, `CMStall` × k²;
  - `CD1` / k²;
  - `[Autopilot] MaxPitchRate` × k, which the round stick's 2.5 then multiplies.
  - Why these: k times the lift is k times the pull at every speed. The pitch moments × k² rotate to the same α k times as fast. `CMq` × k keeps the damping ratio. Each elevator's trim α is unchanged, so level flight and the autopilot's banked hold fly the file's attitudes. `CD1/k²` leaves the induced drag of the harder pull as the file's.
- `Flight::retune` now also copies `lift_scale`, `drag1`, `cm_alpha`, `cm_q`, `cm_de`, `cm_stall` and `max_pitch_rate`, so a mid-ride change (P98) reaches the flight.
- Test `biplane_turn_speed_at_200_percent_pulls_round_near_twice_as_fast` in `crates/game-sim/tests/modifiers.rs`:
  - 100% leaves the file bit-for-bit.
  - Full stick at 90° bank from the level top speed (156.7 ft/s):

    | | 100% | 200% | ratio |
    |---|---|---|---|
    | first second | 81° | 139° | 1.71× |
    | 3 s | 244° | 349° | 1.43× |

  - From 120 ft/s, the first second is 1.58×.
  - With the stick let go at 45°, the 200% plane turns the same (57° vs 58°) and sinks less (11 ft vs 26 ft).
- `crates/openmx/tests/modifiers.rs` gains the new knob in the plane arm of `every_modifier_off_its_default_reaches_the_ride`. menu.rs needed nothing, because it lists `Knob::MENU`.
- Filed **P109**: `retune` still does not copy `CDde`, which Biplane acceleration scales.

Alternatives tried and dropped (scratch probe, 90° bank, New scheme, top speed):

- **`MaxElevator` × k alone** turned slower. It drives α past `CMBreakAngleOfAttack` into the stall break, and the plane bucks.
- **Lift × k plus `MaxPitchRate` × k, moments untouched**: only +12–25%. The pitch overshoots into the stall above 200 ft/s.
- **A "stretched wing"** (`CLRoundOff`/k, `CMBreak` × k, `MaxElevator` × k, `CMStall`/k, `CD1`/k) departed. High α couples roll into yaw, and the plane rolled to 150–180°.
- **`CMq` × k² instead of × k**: within a degree or two of the same turn; × k keeps the damping ratio, so it stayed.
- **`CD1`/k instead of /k²**: within 1° — induced drag is small beside CD0 here.

## What the original does

The turn is the original's code, unchanged. The fade is in the controls listener (`./kit fn` on the plane's controls; see P66/P67's journals), and the PI pitch-rate loop is in the autopilot. Nothing new was read for this task.

## How it was proved

- **100% = original.** `Mods::plane` skips the knob at its default. The test asserts that the file's Debug form (every float to the bit) is unchanged after the knob goes to 250% and back to 100%. The acceptance run and the oracle tests still pass in `./kit check` (1517 tests passed).
- **The port's stock turn = the original's.** This is the acceptance run's bit-exact agreement through tick 691, plus the per-listener oracles. It is not a new capture.
- **The knob's effect** is the test above, on the disc's `biplane.phy`. It ran: its numbers are printed and quoted here.

## Not verified / not 1:1

- **200% is not exactly twice.** It is 1.71× in the first second at top speed, 1.43× over the brief's 3 s, and 1.58× in the first second from 120 ft/s.
  - Why: the turn rate goes with the airspeed, and the harder pull bleeds speed faster (to 57 ft/s vs 82 after 3 s). CD0 is a drag per unit of time that no turn-only field changes.
  - Lowering it would change the top speed, which is Biplane top speed's job. I took this trade over a knob that also changes straight-line flight.
  - The test bounds it at 1.6–2.3× for the first second and ≥1.35× for 3 s.
- **No new PCSX2 flight at top speed.** "The port turns like the original" rests on the acceptance run (bit-exact through tick 691, at 77–115 ft/s) and the per-listener oracles. No banked full-stick turn above about 115 ft/s has been compared against the original frame by frame.
- **Original control scheme on the knife edge:** the knob does nothing there. The original's fade zeroes the elevator past 60° of roll. Below the fade it works (30° bank, first second 2.6×).
- **The 200% plane rolls further under a held bank.** At 60° it rolls to 84° against stock's 64°, because a harder pull's adverse moments overpower a fixed hand. The test holds 90° and 45°; other banks are only probed.
- **`Flight::retune`'s new fields have no test of their own.** A mid-ride change to the knob is not compared against a plane built on it. `crates/openmx/tests/remodify.rs` covers the bike only. P109 is where that test belongs.
- **The knob below 100%** (down to 10%) is not flown in any test. At 10% the wing lifts a tenth, so the plane cannot hold its weight and simply sinks.

## Next

Nothing for this task. P109 picks up `retune`'s `CDde` and a retune-equals-rebuild test for the plane knobs.
