# MD126 — mxvatv car step

2026-11-10. Task: `./kit plan MD126`.

The car's controls (the sixth of MD106a's eleven pieces, filed there as "MD122" before that id went to the
quad loader) ported into `atv_car::Pieces` as SLUS-21104 builds them from MX Unleashed's, and proved
bit-exact with their hook removed under tests/mxvatv_car_frame.rs's frame oracle, alone and with the other
five pieces. Landed; details in 1-mxvatv-car-step-FINAL.md.
