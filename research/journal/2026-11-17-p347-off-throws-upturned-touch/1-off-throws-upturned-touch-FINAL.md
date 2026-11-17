# 1 — off throws upturned touch (FINAL)

Task P347. 2026-11-17.

## What I did

- Decision: with Air Wrecks off, a landing nosed past straight down **is thrown**, on the touch. Why:
  the switch's point (P138, S154) is "turn any way you like in the air, but land right"; S154 already
  moved the rolled no-hander's air wreck to the touch, so this follows that pattern. Riding on after a
  front-wheel landing past vertical was only an artifact of the euler pitch folding back.
- Rule (port's own, `game-sim/src/air.rs`, the wreck test's switched form): `in_air` false, on the
  ground or touching anything, the chassis' up axis below level (`up.y < 0`) → `Tail` if the pitch reads
  up, else `NoseDive`. It sits where the airborne attitude clause sits, so the g-load and ground-pitch
  answers still come first.
- Why the up axis and not the airborne limits at the touch: the up axis needs no `[Wrecks]` numbers.
  The stand-in bike has every limit at zero, so the airborne limits would throw any rolled bike, which
  breaks the S191a stacked-rider tests that ride with the switch off. An upright bike can never trip
  the up-axis rule.
- Side effect, on purpose: a roof landing with the switch off now wrecks on the touch too. Before, no
  clause judged it and it rode on. `tests/air_wrecks.rs`' upside-down jump now asserts that.
- `tests/air_wrecks.rs`: `off_rides_out_a_nose_landing_past_straight_down` became
  `off_throws_a_nose_landing_past_straight_down` (115°: in the air untouched, `NoseDive` on the touch,
  nothing after). The second drop moved from 65° to 85° (just short of vertical, up axis still above
  level), still the same `Front` wreck either way.
- `tests/air.rs`' wreck-test oracle: off may now also answer `NoseDive`/`Tail` where on the ground or
  touching with `up.y < 0`.
- The Air Wrecks help text (`openmx/src/menu_pages.rs`) now says off throws such landings on the touch.
- Files beyond the task's one listed file: `air.rs`, `tests/air.rs`, `menu_pages.rs`. They were needed
  for the rule, its proof and the help text.
- opentennis: no wreck rules there; nothing taken.

## What the original does

The original never reaches this landing: its airborne clause throws a bike past its airborne pitch or
roll limit before it touches (S325, `tests/replay.rs`' `high_nose_first_drops_wreck_as_the_original_does`).
The switch on is unchanged.

## How it was proved

- `tests/air_wrecks.rs` (6 tests) pass: 115° off is a `NoseDive` on the touch, on is a `NoseDive` on
  tick 1; the roof landing off wrecks on its touch tick.
- `tests/air.rs` (wreck-test oracle: on is the original to the bit, off differs only in the allowed
  places), `tests/seat_rivals.rs`, `tests/replay.rs`: pass.

## Not verified / not 1:1

- The rule is the port's own: the original has nothing to compare it to (it throws such bikes in the air).
- Which wreck it gives (`NoseDive` vs `Tail` by the pitch's sign) is my choice, mirroring the airborne
  clause's two answers; not watched on screen.
- A wall or overhang ridden with the bike's up axis below level would now throw with the switch off.
  No shipped track has one, as far as I know; not checked track by track.
- The `air.rs` oracle test passes silently whether or not it found the game's ELF (a pass prints
  nothing), so I did not see it run its 8000 cases here.

## Next

None.
