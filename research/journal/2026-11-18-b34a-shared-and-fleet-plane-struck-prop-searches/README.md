# B34a — shared and fleet plane struck prop searches

2026-11-18. Task: `./kit plan B34a`.

B34 made a ball the lone plane wakes search in the props' pass. B34a does the same for the
shared-seat plane lane (openmx `Plane::step_traffic_shared`, via a now public
`Shared::movers_mut`) and for planes flown together (`flight::frames_together_props_in`,
`vehicle::fly_together_in`, `Shared::set_in`). Proved on B34's images: the fleet's ball 0 list in
the strike tick is image 11066's, field for field; the seat lane and the openmx fleet match the
lone lane bit for bit for 6 ticks. All three fail when switched back to hearing. Done; details in
`1-shared-and-fleet-plane-struck-prop-searches-FINAL.md`.
