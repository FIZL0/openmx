# F58a — held effects item

2026-11-16. Task: `./kit plan F58a`.

This task ported the seventh effects item step (the Picked class's second vtable), which no C15 item
steps, as `effects::Kind::Held`. F58's 20 000-case oracle now draws all seven kinds and passes against
SLUS-21104, covering the latched and unlatched paths, each lighting and dousing. Three mutants each fail
it. PartWalk now runs this kind in Rust too, so no effects item step is handed to the original any more.
