# MD80a — SLUS-21104 captures of the biplane and the Kiowa

2026-11-03. Task: `./kit plan MD80a`. Part of MD80 (research/mxvatv-pc.md "The biplane and the Kiowa (MD80)").

MX vs. ATV Unleashed's two fliers, each in both models, flown in PCSX2 from generated tapes. Each flight
covers take-off, climb, bank, rudder, dive and landing, and is recorded every tick: the flier object, its rigid
body, and its engine/aero/tires (biplane) or rotor/aero (Kiowa). Each flight's start state and RAM image are
kept too. The field map is in research/oracle.txt `# MD80a`, and the recipe and findings are in entry 1. Nothing
is ported. MD80b (the biplane's step) and MD80c (the Kiowa's) replay from these captures.
