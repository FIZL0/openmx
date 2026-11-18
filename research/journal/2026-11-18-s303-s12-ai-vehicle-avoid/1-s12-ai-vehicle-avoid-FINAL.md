# 1 — s12 ai vehicle avoid (FINAL)

Task S303. 2026-11-18.

## What I did

- Nothing new to port. S303 asks for the generic VehicleAI avoidance (`./kit fn 0x002858d0`), ported and
  oracled like `ai::avoid`. S317 (2026-11-12) already did exactly that: `ai::avoid_vehicle` in
  crates/game-sim/src/ai.rs, with the oracle `vehicle_avoidance_matches_over_ten_thousand_passes` in
  crates/game-sim/tests/ai/avoid_vehicle.rs. S303 was filed from S12c before S317 existed and was never closed.
- Its gate ("only once a vehicle that takes it is known to ride in a mode we ship") is also met: S317 found
  every riding brain (mode 5) takes this pass, free09's three AI riders included, and S12e's free09 test
  (tests/ai/sim.rs) runs the port's pass on them.
- Closing S303 with no code change. Decided alone (unattended run): redoing the port would only duplicate S317.

## What the original does

See S317's journal (2026-11-12-s317-generic-vehicle-avoidance), "What the original does".

## How it was proved

- Re-ran S317's function oracle on this branch: `./kit check -- -p game-sim --test ai vehicle_avoidance`,
  10,000 generated passes against the original in Ee, all matched (TESTS PASSED).

## Not verified / not 1:1

- Nothing new was ported here, so nothing new is unproven. S317's own list still stands (scene walk and
  bundle sweep are caller callouts, class registry not ported, n = 0 feelers unsupported, debug draw left
  out); see its journal.

## Next

none — S303 is a duplicate of S317, closed.
