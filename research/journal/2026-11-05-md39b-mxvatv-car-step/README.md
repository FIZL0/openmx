# MD39b — mxvatv car step

2026-11-05. Task: `./kit plan MD39b`.

Part 1 of MD39b. `game_sim::car::Car::read_mxvatv` reads MX vs. ATV's car files as SLUS-21104 does: MX
Unleashed's car loader wrapped in MXVehicle's `[Wrecks]` and `[Camera]` loaders, chosen by a five-car
`MxvatvMachine` switch. The SLUS-21104 field map (research/oracle.txt `# MD39b`) is proved on MD39a's buggy,
baja and truck RAM images: 1,271 to 1,283 loader words per car, bit for bit (tests/mxvatv_car_fields.rs).
MD39a's captures had gone from this slot. I recovered three of the five states from my emulator copy's save
slots; the golf cart and sand rail states, the tape and every csv are lost. So the step and the replay are
not done. They are filed as C29 (remake the captures), MD106 (the car's whole frame) and MD107 (the five
drives replayed), and MD39b is blocked on them. Details in `1-mxvatv-car-step-PART.md`.
